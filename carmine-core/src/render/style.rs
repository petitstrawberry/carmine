use ego_tree::NodeRef as EgoNodeRef;
use scraper::{Html, Node as ScraperNode};

use crate::render::css::{self, Declaration, MatchMap, PseudoElement, PseudoMatchMap};
use cssparser::{Parser as CssParser, ParserInput, Token};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Length {
    Px(f32),
    Percent(f32),
    Vw(f32),
    Vh(f32),
    Clamp {
        min_px: f32,
        preferred_px: f32,
        preferred_vw: f32,
        max_px: f32,
    },
    Auto,
    Zero,
}

impl Default for Length {
    fn default() -> Self {
        Length::Zero
    }
}

impl Length {
    pub fn px(v: f32) -> Self {
        if v == 0.0 {
            Length::Zero
        } else {
            Length::Px(v)
        }
    }

    pub fn parse(s: &str) -> Length {
        parse_length_token(s)
    }

    pub fn resolve_px(&self, container: f32, viewport: f32) -> f32 {
        match self {
            Length::Px(v) => *v,
            Length::Percent(v) => container * v,
            Length::Vw(v) => v * viewport / 100.0,
            Length::Vh(v) => v * viewport / 100.0,
            Length::Clamp {
                min_px,
                preferred_px,
                preferred_vw,
                max_px,
            } => {
                let preferred = if *preferred_vw != 0.0 {
                    preferred_vw * viewport / 100.0
                } else {
                    *preferred_px
                };
                preferred.max(*min_px).min(*max_px)
            }
            Length::Zero => 0.0,
            Length::Auto => 0.0,
        }
    }

    pub fn is_auto(&self) -> bool {
        matches!(self, Length::Auto)
    }

    pub fn approx_px(&self) -> f32 {
        match self {
            Length::Px(v) => *v,
            Length::Percent(v) => *v * ROOT_FONT_SIZE,
            Length::Vw(v) => *v,
            Length::Vh(v) => *v,
            Length::Clamp {
                min_px,
                preferred_px,
                max_px,
                ..
            } => preferred_px.max(*min_px).min(*max_px),
            Length::Auto => ROOT_FONT_SIZE,
            Length::Zero => 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BoxSizing {
    ContentBox,
    BorderBox,
}

#[derive(Clone, Debug)]
pub struct ComputedStyle {
    pub display: Display,
    pub visibility: Visibility,
    pub font_size: f32,
    pub font_weight: u32,
    pub color: Color,
    pub background_color: Option<Background>,
    pub background_image_src: Option<String>,
    pub background_repeat_x: bool,
    pub background_repeat_y: bool,
    pub background_position_x: BackgroundPositionAxis,
    pub background_position_y: BackgroundPositionAxis,
    pub margin_top: Length,
    pub margin_bottom: Length,
    pub margin_left: Length,
    pub margin_right: Length,
    pub padding_top: Length,
    pub padding_right: Length,
    pub padding_bottom: Length,
    pub padding_left: Length,
    pub flex_direction: FlexDirection,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub flex_wrap: FlexWrap,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Length,
    pub gap: Length,
    pub border_width: f32,
    pub border_color: Option<Color>,
    pub border_left_width: f32,
    pub border_left_color: Option<Color>,
    pub border_radius: f32,
    pub width: Length,
    pub height: Length,
    pub max_width: Length,
    pub min_height: Length,
    pub box_sizing: BoxSizing,
    pub letter_spacing: f32,
    pub text_transform: TextTransform,
    pub text_align: TextAlign,
    pub text_decoration: TextDecoration,
    pub line_height: LineHeight,
    pub white_space: WhiteSpace,
    pub border_top_width: f32,
    pub border_top_color: Option<Color>,
    pub border_right_width: f32,
    pub border_right_color: Option<Color>,
    pub border_bottom_width: f32,
    pub border_bottom_color: Option<Color>,
    pub grid_template_columns: String,
    pub box_shadow: Option<BoxShadow>,
    pub img_src: Option<String>,
    pub img_width: Option<u32>,
    pub img_height: Option<u32>,
    pub table_colspan: u16,
    pub table_rowspan: u16,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextAlign {
    Start,
    Left,
    Right,
    Center,
    Justify,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextDecoration {
    None,
    Underline,
    LineThrough,
    Overline,
}

/// CSS line-height value: `Normal` = 1.2 multiplier, `Number(f32)` = multiplier, `Px(f32)` = absolute.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineHeight {
    Normal,
    Number(f32),
    Px(f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WhiteSpace {
    Normal,
    Nowrap,
    Pre,
    PreWrap,
}

impl LineHeight {
    /// Resolve to an absolute pixel value for the given font size.
    pub fn resolve_px(self, font_size: f32) -> f32 {
        match self {
            LineHeight::Normal => font_size * 1.2,
            LineHeight::Number(n) => font_size * n,
            LineHeight::Px(px) => px,
        }
    }
}

#[derive(Clone, Debug)]
pub struct BoxShadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: Color,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextTransform {
    None,
    Uppercase,
    Lowercase,
    Capitalize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Display {
    Block,
    Inline,
    Flex,
    InlineFlex,
    Grid,
    InlineGrid,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Visibility {
    Visible,
    Hidden,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FlexDirection {
    Row,
    Column,
    RowReverse,
    ColumnReverse,
    Unspecified,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum JustifyContent {
    FlexStart,
    Center,
    FlexEnd,
    SpaceBetween,
    SpaceAround,
    Unspecified,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AlignItems {
    FlexStart,
    Center,
    FlexEnd,
    Stretch,
    Unspecified,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FlexWrap {
    NoWrap,
    Wrap,
    Unspecified,
}

#[derive(Clone, Copy, Debug)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Clone, Debug)]
pub enum Background {
    Solid(Color),
    Linear {
        angle_deg: f32,
        stops: Vec<(Color, f32)>,
    },
    Radial {
        center_x: f32,
        center_y: f32,
        stops: Vec<(Color, f32)>,
    },
    RepeatingLinear {
        angle_deg: f32,
        color: Color,
        stripe_width: f32,
        period: f32,
    },
    Layers(Vec<Background>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BackgroundPositionAxis {
    Start(Length),
    Center(Length),
    End(Length),
}

impl Color {
    pub const BLACK: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };
    pub const WHITE: Color = Color {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    pub const CRIMSON: Color = Color {
        r: 220,
        g: 20,
        b: 60,
        a: 255,
    };
    pub const GRAY: Color = Color {
        r: 128,
        g: 128,
        b: 128,
        a: 255,
    };
}

#[derive(Debug)]
pub struct StyledNode {
    pub kind: StyledKind,
    pub style: ComputedStyle,
    pub pseudo_before: Option<ComputedStyle>,
    pub pseudo_after: Option<ComputedStyle>,
    pub img_src: Option<String>,
    pub children: Vec<StyledNode>,
}

#[derive(Debug)]
pub enum StyledKind {
    Element { tag: String },
    Text(String),
    Document,
}

const ROOT_FONT_SIZE: f32 = 16.0;

pub fn compute_styles(
    html: &Html,
    matches: &MatchMap,
    pseudo_matches: &PseudoMatchMap,
    viewport_w: u32,
    body_margin: f32,
) -> StyledNode {
    let root = html.tree.root();
    cascade(
        &root,
        matches,
        pseudo_matches,
        &default_block(),
        viewport_w as f32,
        body_margin,
    )
}

fn cascade(
    node_ref: &EgoNodeRef<ScraperNode>,
    matches: &MatchMap,
    pseudo_matches: &PseudoMatchMap,
    parent: &ComputedStyle,
    viewport_w: f32,
    body_margin: f32,
) -> StyledNode {
    match node_ref.value() {
        ScraperNode::Document | ScraperNode::Fragment => {
            let children = walk_children(
                node_ref,
                matches,
                pseudo_matches,
                parent,
                viewport_w,
                body_margin,
            );
            StyledNode {
                kind: StyledKind::Document,
                style: parent.clone(),
                pseudo_before: None,
                pseudo_after: None,
                img_src: None,
                children,
            }
        }
        ScraperNode::Element(el) => {
            let tag = el.name().to_string();
            let id = node_ref.id();
            let mut style = default_for_tag(&tag, body_margin);

            apply_html_attributes(&tag, el, &mut style);

            let matched_count = matches.get(&id).map_or(0, |d| d.len());
            if matched_count > 0 {
                println!(
                    "[carmine] cascade <{}>: {} decls matched",
                    tag, matched_count
                );
            }

            if let Some(decls) = matches.get(&id) {
                let mut sorted = decls.clone();
                sorted.sort_by(|a, b| (a.0.important as u8, a.1).cmp(&(b.0.important as u8, b.1)));
                for (decl, _) in &sorted {
                    if decl.property == "font-size" {
                        let len = parse_length_token(&decl.value);
                        style.font_size = resolve_font_size(len, parent.font_size, viewport_w);
                        println!(
                            "[carmine]   <{}> font-size: {} -> {}",
                            tag, decl.value, style.font_size
                        );
                    }
                }
                for (decl, _) in &sorted {
                    if decl.property != "font-size" {
                        apply_declaration(&mut style, decl);
                    }
                }
            }

            let mut css_sets_font_size =
                matches.get(&id).map_or(false, |d| {
                    d.iter().any(|(decl, _)| decl.property == "font-size")
                }) || el.attr("style").map_or(false, |s| s.contains("font-size"));
            let css_sets_color = matches.get(&id).map_or(false, |d| {
                d.iter().any(|(decl, _)| decl.property == "color")
            }) || el.attr("style").map_or(false, |s| s.contains("color"));
            let css_sets_font_weight = matches.get(&id).map_or(false, |d| {
                d.iter().any(|(decl, _)| decl.property == "font-weight")
            }) || el
                .attr("style")
                .map_or(false, |s| s.contains("font-weight"));
            let css_sets_letter_spacing = matches.get(&id).map_or(false, |d| {
                d.iter().any(|(decl, _)| decl.property == "letter-spacing")
            });
            let css_sets_text_transform = matches.get(&id).map_or(false, |d| {
                d.iter().any(|(decl, _)| decl.property == "text-transform")
            });
            let css_sets_visibility =
                matches.get(&id).map_or(false, |d| {
                    d.iter().any(|(decl, _)| decl.property == "visibility")
                }) || el.attr("style").map_or(false, |s| s.contains("visibility"));
            let mut css_sets_white_space = matches.get(&id).map_or(false, |d| {
                d.iter()
                    .any(|(decl, _)| decl.property == "white-space" || decl.property == "text-wrap")
            }) || el.attr("style").map_or(false, |s| {
                s.contains("white-space") || s.contains("text-wrap")
            });

            if let Some(inline) = el.attr("style") {
                let inline_decls = css::parse_declarations(inline);
                if !css_sets_font_size {
                    css_sets_font_size = inline_decls.iter().any(|d| d.property == "font-size");
                }
                if !css_sets_white_space {
                    css_sets_white_space = inline_decls
                        .iter()
                        .any(|d| d.property == "white-space" || d.property == "text-wrap");
                }
                for decl in &inline_decls {
                    if decl.property == "font-size" {
                        let len = parse_length_token(&decl.value);
                        style.font_size = resolve_font_size(len, parent.font_size, viewport_w);
                    }
                }
                for decl in &inline_decls {
                    if decl.property != "font-size" {
                        apply_declaration(&mut style, decl);
                    }
                }
            }

            if !css_sets_font_size && !is_heading_tag(&tag) {
                style.font_size = parent.font_size;
            }
            if !css_sets_color {
                style.color = parent.color;
            }
            if !css_sets_font_weight {
                style.font_weight = parent.font_weight;
            }
            if !css_sets_letter_spacing {
                style.letter_spacing = parent.letter_spacing;
            }
            if !css_sets_text_transform {
                style.text_transform = parent.text_transform;
            }
            if !css_sets_visibility {
                style.visibility = parent.visibility;
            }
            if !css_sets_white_space {
                style.white_space = parent.white_space;
            }

            resolve_em_lengths(&mut style);

            println!(
                "[carmine]   <{}> final: font_size={}, color=({}, {}, {}), display={:?}, bg={}",
                tag,
                style.font_size,
                style.color.r,
                style.color.g,
                style.color.b,
                style.display,
                style.background_color.is_some()
            );

            let pseudo_before =
                compute_pseudo_style(id, pseudo_matches, PseudoElement::Before, &style);
            let pseudo_after =
                compute_pseudo_style(id, pseudo_matches, PseudoElement::After, &style);

            let input_type = el.attr("type").unwrap_or("").to_lowercase();
            let input_value = el.attr("value").unwrap_or("").to_string();
            let is_hidden_input = tag.eq_ignore_ascii_case("input") && input_type == "hidden";
            if is_hidden_input {
                style.display = Display::None;
            }
            let show_input_text = tag.eq_ignore_ascii_case("input")
                && !is_hidden_input
                && matches!(input_type.as_str(), "submit" | "button" | "reset" | "")
                && !input_value.is_empty();

            if show_input_text && style.width.is_auto() {
                let text_width = input_value.chars().count() as f32 * style.font_size * 0.65;
                let horizontal_padding =
                    style.padding_left.approx_px() + style.padding_right.approx_px();
                style.width = Length::px((text_width + horizontal_padding + 12.0).max(48.0));
            }

            let mut children = if style.display == Display::None {
                Vec::new()
            } else {
                walk_children(
                    node_ref,
                    matches,
                    pseudo_matches,
                    &style,
                    viewport_w,
                    body_margin,
                )
            };

            if show_input_text {
                children.push(StyledNode {
                    kind: StyledKind::Text(input_value),
                    style: ComputedStyle {
                        display: Display::Inline,
                        visibility: style.visibility,
                        font_size: style.font_size,
                        color: style.color,
                        text_align: style.text_align,
                        text_decoration: style.text_decoration,
                        line_height: style.line_height,
                        white_space: style.white_space,
                        ..default_block()
                    },
                    pseudo_before: None,
                    pseudo_after: None,
                    img_src: None,
                    children: Vec::new(),
                });
            }

            let img_src = if tag.eq_ignore_ascii_case("img") {
                el.attr("src").map(|s| s.to_string())
            } else {
                None
            };

            StyledNode {
                kind: StyledKind::Element { tag },
                style,
                pseudo_before,
                pseudo_after,
                img_src,
                children,
            }
        }
        ScraperNode::Text(text) => {
            let inherited = ComputedStyle {
                display: Display::Inline,
                visibility: parent.visibility,
                font_size: parent.font_size,
                font_weight: parent.font_weight,
                color: parent.color,
                letter_spacing: parent.letter_spacing,
                text_transform: parent.text_transform,
                text_align: parent.text_align,
                text_decoration: parent.text_decoration,
                line_height: parent.line_height,
                white_space: parent.white_space,
                ..default_block()
            };
            StyledNode {
                kind: StyledKind::Text(text.to_string()),
                style: inherited,
                pseudo_before: None,
                pseudo_after: None,
                img_src: None,
                children: Vec::new(),
            }
        }
        _ => StyledNode {
            kind: StyledKind::Document,
            style: parent.clone(),
            pseudo_before: None,
            pseudo_after: None,
            img_src: None,
            children: Vec::new(),
        },
    }
}

fn compute_pseudo_style(
    id: ego_tree::NodeId,
    pseudo_matches: &PseudoMatchMap,
    pseudo: PseudoElement,
    parent: &ComputedStyle,
) -> Option<ComputedStyle> {
    let mut decls: Vec<(Declaration, u32)> = pseudo_matches
        .get(&id)?
        .iter()
        .filter(|(_, _, p)| *p == pseudo)
        .map(|(decl, spec, _)| (decl.clone(), *spec))
        .collect();

    if decls.is_empty() {
        return None;
    }

    decls.sort_by(|a, b| (a.0.important as u8, a.1).cmp(&(b.0.important as u8, b.1)));

    let mut style = ComputedStyle {
        display: Display::Inline,
        visibility: parent.visibility,
        font_size: parent.font_size,
        font_weight: parent.font_weight,
        color: parent.color,
        letter_spacing: parent.letter_spacing,
        text_transform: parent.text_transform,
        text_align: parent.text_align,
        text_decoration: parent.text_decoration,
        line_height: parent.line_height,
        white_space: parent.white_space,
        ..default_block()
    };

    for (decl, _) in &decls {
        if decl.property != "content" {
            apply_declaration(&mut style, decl);
        }
    }

    Some(style)
}

fn walk_children(
    node_ref: &EgoNodeRef<ScraperNode>,
    matches: &MatchMap,
    pseudo_matches: &PseudoMatchMap,
    parent: &ComputedStyle,
    viewport_w: f32,
    body_margin: f32,
) -> Vec<StyledNode> {
    node_ref
        .children()
        .filter(|c| !is_ignored(c.value()))
        .map(|c| cascade(&c, matches, pseudo_matches, parent, viewport_w, body_margin))
        .collect()
}

fn is_ignored(node: &ScraperNode) -> bool {
    matches!(
        node,
        ScraperNode::Comment(_) | ScraperNode::Doctype(_) | ScraperNode::ProcessingInstruction(_)
    )
}

fn is_heading_tag(tag: &str) -> bool {
    matches!(
        tag.to_lowercase().as_str(),
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
    )
}

fn apply_html_attributes(tag: &str, el: &scraper::node::Element, style: &mut ComputedStyle) {
    match tag.to_lowercase().as_str() {
        "body" | "div" => apply_legacy_background_attrs(el, style),
        "table" => {
            apply_html_box_dimensions(el, style);
            if let Some(cp) = el.attr("cellpadding") {
                if let Ok(px) = cp.trim().parse::<f32>() {
                    let len = Length::px(px);
                    style.padding_top = len;
                    style.padding_right = len;
                    style.padding_bottom = len;
                    style.padding_left = len;
                }
            }
            if let Some(_) = el.attr("cellspacing") {
                if let Some(cs) = el.attr("cellspacing") {
                    if let Ok(px) = cs.trim().parse::<f32>() {
                        style.gap = Length::px(px);
                    }
                }
            }
        }
        "img" => {
            apply_html_box_dimensions(el, style);
        }
        "td" | "th" => {
            apply_html_box_dimensions(el, style);
            if let Some(colspan) = el.attr("colspan").and_then(parse_span_value) {
                style.table_colspan = colspan;
            }
            if let Some(rowspan) = el.attr("rowspan").and_then(parse_span_value) {
                style.table_rowspan = rowspan;
            }
            if let Some(align) = el.attr("align") {
                match align.trim().to_ascii_lowercase().as_str() {
                    "center" | "middle" => {
                        style.text_align = TextAlign::Center;
                        style.align_items = AlignItems::Center;
                    }
                    "right" => {
                        style.text_align = TextAlign::Right;
                        style.align_items = AlignItems::FlexEnd;
                    }
                    "left" => {
                        style.text_align = TextAlign::Left;
                        style.align_items = AlignItems::FlexStart;
                    }
                    _ => {}
                };
            }
            if el.attr("nowrap").is_some() {
                style.white_space = WhiteSpace::Nowrap;
            }
        }
        "span" => {
            if el.attr("nowrap").is_some() {
                style.white_space = WhiteSpace::Nowrap;
            }
        }
        _ => {}
    }
}

fn apply_html_box_dimensions(el: &scraper::node::Element, style: &mut ComputedStyle) {
    if let Some(width) = el.attr("width").and_then(parse_html_length_attr) {
        style.width = width;
    }
    if let Some(height) = el.attr("height").and_then(parse_html_length_attr) {
        style.height = height;
    }
}

fn parse_html_length_attr(value: &str) -> Option<Length> {
    let value = value.trim().trim_matches(|ch| ch == '"' || ch == '\'');
    if value.is_empty() {
        return None;
    }
    if let Some(percent) = value.strip_suffix('%') {
        let percent = percent.trim().parse::<f32>().ok()?;
        return Some(Length::Percent((percent / 100.0).max(0.0)));
    }
    let number = value.parse::<f32>().ok()?;
    Some(Length::px(number.max(0.0)))
}

fn parse_span_value(value: &str) -> Option<u16> {
    let value = value.trim().trim_matches(|ch| ch == '"' || ch == '\'');
    let parsed = value.parse::<u16>().ok()?;
    Some(parsed.max(1))
}

fn apply_legacy_background_attrs(el: &scraper::node::Element, style: &mut ComputedStyle) {
    if let Some(background) = el.attr("background") {
        let background = background.trim();
        if !background.is_empty() {
            style.background_image_src = Some(background.to_string());
            style.background_repeat_x = true;
            style.background_repeat_y = true;
            style.background_position_x = BackgroundPositionAxis::Start(Length::Zero);
            style.background_position_y = BackgroundPositionAxis::Start(Length::Zero);
        }
    }
    if let Some(bgcolor) = el.attr("bgcolor").and_then(parse_color) {
        style.background_color = Some(Background::Solid(bgcolor));
    }
}

fn resolve_font_size(len: Length, parent_font_size: f32, viewport_w: f32) -> f32 {
    match len {
        Length::Auto | Length::Zero => parent_font_size,
        _ => len.resolve_px(parent_font_size, viewport_w),
    }
}

fn length_to_clamp_px(len: Length) -> f32 {
    match len {
        Length::Px(v) => v,
        Length::Percent(v) => v * ROOT_FONT_SIZE,
        Length::Vw(v) => v,
        Length::Vh(v) => v,
        Length::Clamp { preferred_px, .. } => preferred_px,
        Length::Auto | Length::Zero => 0.0,
    }
}

fn resolve_em_lengths(style: &mut ComputedStyle) {
    let fs = style.font_size;
    let resolve = |l: Length| -> Length {
        match l {
            Length::Px(v) => Length::Px(v),
            Length::Percent(v) => Length::Percent(v),
            Length::Vw(v) => Length::Vw(v),
            Length::Vh(v) => Length::Vh(v),
            Length::Clamp {
                min_px,
                preferred_px,
                preferred_vw,
                max_px,
            } => Length::Clamp {
                min_px,
                preferred_px,
                preferred_vw,
                max_px,
            },
            Length::Auto => Length::Auto,
            Length::Zero => Length::Zero,
        }
    };
    style.margin_top = resolve(style.margin_top);
    style.margin_bottom = resolve(style.margin_bottom);
    style.margin_left = resolve(style.margin_left);
    style.margin_right = resolve(style.margin_right);
    let _ = fs;
    style.gap = resolve(style.gap);
    let _ = fs;
}

fn default_block() -> ComputedStyle {
    ComputedStyle {
        display: Display::Block,
        visibility: Visibility::Visible,
        font_size: ROOT_FONT_SIZE,
        font_weight: 400,
        color: Color::BLACK,
        background_color: None,
        background_image_src: None,
        background_repeat_x: false,
        background_repeat_y: false,
        background_position_x: BackgroundPositionAxis::Start(Length::Zero),
        background_position_y: BackgroundPositionAxis::Start(Length::Zero),
        margin_top: Length::Zero,
        margin_bottom: Length::Zero,
        margin_left: Length::Zero,
        margin_right: Length::Zero,
        padding_top: Length::Zero,
        padding_right: Length::Zero,
        padding_bottom: Length::Zero,
        padding_left: Length::Zero,
        flex_direction: FlexDirection::Unspecified,
        justify_content: JustifyContent::Unspecified,
        align_items: AlignItems::Unspecified,
        flex_wrap: FlexWrap::Unspecified,
        flex_grow: 0.0,
        flex_shrink: 1.0,
        flex_basis: Length::Auto,
        gap: Length::Zero,
        border_width: 0.0,
        border_color: None,
        border_left_width: 0.0,
        border_left_color: None,
        border_radius: 0.0,
        width: Length::Auto,
        height: Length::Auto,
        max_width: Length::Auto,
        min_height: Length::Auto,
        box_sizing: BoxSizing::ContentBox,
        letter_spacing: 0.0,
        text_transform: TextTransform::None,
        text_align: TextAlign::Start,
        text_decoration: TextDecoration::None,
        line_height: LineHeight::Normal,
        white_space: WhiteSpace::Normal,
        border_top_width: 0.0,
        border_top_color: None,
        border_right_width: 0.0,
        border_right_color: None,
        border_bottom_width: 0.0,
        border_bottom_color: None,
        grid_template_columns: String::new(),
        box_shadow: None,
        img_src: None,
        img_width: None,
        img_height: None,
        table_colspan: 1,
        table_rowspan: 1,
    }
}

#[cfg(test)]
pub(crate) fn default_block_for_test() -> ComputedStyle {
    default_block()
}

fn default_for_tag(tag: &str, body_margin: f32) -> ComputedStyle {
    let base = default_block();
    match tag {
        "html" => base,
        "body" => {
            let margin = body_margin.max(0.0);
            ComputedStyle {
                margin_top: Length::px(margin),
                margin_right: Length::px(margin),
                margin_bottom: Length::px(margin),
                margin_left: Length::px(margin),
                ..base
            }
        }
        "head" | "title" | "meta" | "link" | "style" | "script" => ComputedStyle {
            display: Display::None,
            ..base
        },
        "h1" => ComputedStyle {
            font_size: 32.0,
            font_weight: 700,
            margin_top: Length::px(21.0),
            margin_bottom: Length::px(21.0),
            color: Color::CRIMSON,
            ..base
        },
        "h2" => ComputedStyle {
            font_size: 24.0,
            font_weight: 700,
            margin_top: Length::px(19.0),
            margin_bottom: Length::px(19.0),
            ..base
        },
        "h3" => ComputedStyle {
            font_size: 19.0,
            font_weight: 700,
            margin_top: Length::px(18.0),
            margin_bottom: Length::px(18.0),
            ..base
        },
        "p" => ComputedStyle {
            margin_top: Length::px(16.0),
            margin_bottom: Length::px(16.0),
            ..base
        },
        "div" | "section" | "article" | "header" | "footer" | "main" | "nav" | "aside" | "form" => {
            base
        }
        "ul" | "ol" => ComputedStyle {
            margin_top: Length::px(16.0),
            margin_bottom: Length::px(16.0),
            ..base
        },
        "li" => base,
        "a" => ComputedStyle {
            display: Display::Inline,
            color: Color {
                r: 0,
                g: 0,
                b: 238,
                a: 255,
            },
            ..base
        },
        "b" | "strong" => ComputedStyle {
            display: Display::Inline,
            font_weight: 700,
            ..base
        },
        "center" => ComputedStyle {
            text_align: TextAlign::Center,
            align_items: AlignItems::Center,
            ..base
        },
        "input" | "button" | "select" | "textarea" => ComputedStyle {
            display: Display::Inline,
            white_space: WhiteSpace::Nowrap,
            border_width: 1.0,
            border_color: Some(Color {
                r: 118,
                g: 118,
                b: 118,
                a: 255,
            }),
            padding_top: Length::px(3.0),
            padding_bottom: Length::px(3.0),
            padding_left: Length::px(6.0),
            padding_right: Length::px(6.0),
            background_color: Some(Background::Solid(Color::WHITE)),
            height: Length::px(20.0),
            box_sizing: BoxSizing::BorderBox,
            ..base
        },
        "table" => ComputedStyle {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            width: Length::Auto,
            ..base
        },
        "thead" | "tbody" | "tfoot" => ComputedStyle {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            ..base
        },
        "tr" => ComputedStyle {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            ..base
        },
        "td" | "th" => ComputedStyle {
            display: Display::Block,
            padding_top: Length::px(1.0),
            padding_bottom: Length::px(1.0),
            padding_left: Length::px(8.0),
            padding_right: Length::px(8.0),
            ..base
        },
        _ => ComputedStyle {
            display: Display::Inline,
            ..base
        },
    }
}

fn apply_declaration(style: &mut ComputedStyle, decl: &Declaration) {
    let value = decl.value.trim();
    match decl.property.as_str() {
        "color" => {
            if let Some(c) = parse_color(value) {
                style.color = c;
            }
        }
        "background-color" => {
            style.background_color = parse_background(value);
        }
        "background-image" => {
            style.background_image_src = parse_background_image_url(value);
        }
        "background-repeat" => {
            if let Some((rx, ry)) = parse_background_repeat(value) {
                style.background_repeat_x = rx;
                style.background_repeat_y = ry;
            }
        }
        "background-position" => {
            if let Some((px, py)) = parse_background_position(value) {
                style.background_position_x = px;
                style.background_position_y = py;
            }
        }
        "background" => {
            if let Some(result) = parse_background_shorthand(value) {
                style.background_color = result.color;
                style.background_image_src = result.image_src;
                style.background_repeat_x = result.repeat_x;
                style.background_repeat_y = result.repeat_y;
                style.background_position_x = result.position_x;
                style.background_position_y = result.position_y;
            } else {
                style.background_color = parse_background(value);
            }
        }
        "font-size" => {}
        "font-weight" => {
            style.font_weight = parse_font_weight(value);
        }
        "display" => {
            if let Some(d) = parse_display(value) {
                style.display = d;
            }
        }
        "flex-direction" => {
            style.flex_direction = parse_flex_direction(value);
        }
        "justify-content" => {
            style.justify_content = parse_justify_content(value);
        }
        "align-items" => {
            style.align_items = parse_align_items(value);
        }
        "flex-wrap" => {
            style.flex_wrap = parse_flex_wrap(value);
        }
        "flex-grow" => {
            if let Ok(v) = value.parse::<f32>() {
                style.flex_grow = v.max(0.0);
            }
        }
        "flex-shrink" => {
            if let Ok(v) = value.parse::<f32>() {
                style.flex_shrink = v.max(0.0);
            }
        }
        "flex-basis" => {
            style.flex_basis = parse_length_token(value);
        }
        "flex" => {
            parse_flex_shorthand(style, value);
        }
        "visibility" => {
            style.visibility = match value {
                "hidden" | "collapse" => Visibility::Hidden,
                _ => Visibility::Visible,
            };
        }
        "gap" => {
            style.gap = parse_length_token(value);
        }
        "margin" => {
            let (t, b, l, r) = parse_margin_shorthand(value);
            style.margin_top = t;
            style.margin_bottom = b;
            style.margin_left = l;
            style.margin_right = r;
        }
        "margin-top" => style.margin_top = parse_length_token(value),
        "margin-bottom" => style.margin_bottom = parse_length_token(value),
        "margin-left" => style.margin_left = parse_length_token(value),
        "margin-right" => style.margin_right = parse_length_token(value),
        "margin-inline-start" => style.margin_left = parse_length_token(value),
        "margin-inline-end" => style.margin_right = parse_length_token(value),
        "margin-block-start" => style.margin_top = parse_length_token(value),
        "margin-block-end" => style.margin_bottom = parse_length_token(value),
        "padding" => {
            let parts: Vec<&str> = value.split_whitespace().collect();
            let parse = |s: &str| parse_length_token(s);
            match parts.len() {
                1 => {
                    let v = parse(parts[0]);
                    style.padding_top = v;
                    style.padding_right = v;
                    style.padding_bottom = v;
                    style.padding_left = v;
                }
                2 => {
                    let v = parse(parts[0]);
                    let h = parse(parts[1]);
                    style.padding_top = v;
                    style.padding_bottom = v;
                    style.padding_left = h;
                    style.padding_right = h;
                }
                3 => {
                    style.padding_top = parse(parts[0]);
                    let h = parse(parts[1]);
                    style.padding_left = h;
                    style.padding_right = h;
                    style.padding_bottom = parse(parts[2]);
                }
                4 => {
                    style.padding_top = parse(parts[0]);
                    style.padding_right = parse(parts[1]);
                    style.padding_bottom = parse(parts[2]);
                    style.padding_left = parse(parts[3]);
                }
                _ => {}
            }
        }
        "border" => {
            let (w, c) = parse_border_shorthand(value);
            style.border_width = w;
            style.border_color = c;
        }
        "padding-inline-start" => style.padding_left = parse_length_token(value),
        "padding-inline-end" => style.padding_right = parse_length_token(value),
        "padding-block-start" => style.padding_top = parse_length_token(value),
        "padding-block-end" => style.padding_bottom = parse_length_token(value),
        "border-left" => {
            let (w, c) = parse_border_shorthand(value);
            style.border_left_width = w;
            style.border_left_color = c;
        }
        "border-width" => {
            if let Some(v) = parse_simple_px(value) {
                style.border_width = v;
            }
        }
        "border-color" => {
            style.border_color = parse_color(value);
        }
        "border-radius" => {
            if let Some(v) = parse_border_radius(value) {
                style.border_radius = v;
            }
        }
        "width" => {
            style.width = parse_length_token(value);
        }
        "height" => {
            style.height = parse_length_token(value);
        }
        "max-width" => {
            style.max_width = parse_length_token(value);
        }
        "min-height" => {
            style.min_height = parse_length_token(value);
        }
        "box-sizing" => {
            style.box_sizing = match value.trim() {
                "border-box" => BoxSizing::BorderBox,
                _ => BoxSizing::ContentBox,
            };
        }
        "letter-spacing" => {
            if let Some(v) = parse_simple_px(value) {
                style.letter_spacing = v;
            }
        }
        "text-transform" => {
            style.text_transform = match value.trim() {
                "uppercase" => TextTransform::Uppercase,
                "lowercase" => TextTransform::Lowercase,
                "capitalize" => TextTransform::Capitalize,
                _ => TextTransform::None,
            };
        }
        "grid-template-columns" => {
            style.grid_template_columns = value.trim().to_string();
        }
        "box-shadow" => {
            style.box_shadow = parse_box_shadow(value);
        }
        "text-align" => {
            style.text_align = match value.trim() {
                "center" => TextAlign::Center,
                "right" => TextAlign::Right,
                "left" => TextAlign::Left,
                "justify" => TextAlign::Justify,
                _ => TextAlign::Start,
            };
        }
        "text-decoration" => {
            let v = value.trim();
            if v.contains("underline") {
                style.text_decoration = TextDecoration::Underline;
            } else if v.contains("line-through") {
                style.text_decoration = TextDecoration::LineThrough;
            } else if v.contains("overline") {
                style.text_decoration = TextDecoration::Overline;
            } else {
                style.text_decoration = TextDecoration::None;
            }
        }
        "line-height" => {
            style.line_height = parse_line_height(value);
        }
        "white-space" | "text-wrap" => {
            style.white_space = match value.trim() {
                "nowrap" => WhiteSpace::Nowrap,
                "pre" => WhiteSpace::Pre,
                "pre-wrap" => WhiteSpace::PreWrap,
                _ => WhiteSpace::Normal,
            };
        }
        "border-top" => {
            let (w, c) = parse_border_shorthand(value);
            style.border_top_width = w;
            style.border_top_color = c;
        }
        "border-right" => {
            let (w, c) = parse_border_shorthand(value);
            style.border_right_width = w;
            style.border_right_color = c;
        }
        "border-bottom" => {
            let (w, c) = parse_border_shorthand(value);
            style.border_bottom_width = w;
            style.border_bottom_color = c;
        }
        "border-top-width" => {
            if let Some(v) = parse_simple_px(value) {
                style.border_top_width = v;
            }
        }
        "border-right-width" => {
            if let Some(v) = parse_simple_px(value) {
                style.border_right_width = v;
            }
        }
        "border-bottom-width" => {
            if let Some(v) = parse_simple_px(value) {
                style.border_bottom_width = v;
            }
        }
        "border-top-color" => {
            style.border_top_color = parse_color(value);
        }
        "border-right-color" => {
            style.border_right_color = parse_color(value);
        }
        "border-bottom-color" => {
            style.border_bottom_color = parse_color(value);
        }
        _ => {}
    }
}

fn parse_length_token(value: &str) -> Length {
    let value = value.trim();

    if let Some(inner) = extract_css_function(value, "clamp") {
        return parse_clamp(&inner);
    }
    if let Some(inner) = extract_css_function(value, "min") {
        let parts: Vec<&str> = inner.split(',').collect();
        return parts
            .iter()
            .map(|p| parse_length_token(p.trim()))
            .min_by(|a, b| {
                a.approx_px()
                    .partial_cmp(&b.approx_px())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(Length::Zero);
    }
    if let Some(inner) = extract_css_function(value, "max") {
        let parts: Vec<&str> = inner.split(',').collect();
        return parts
            .iter()
            .map(|p| parse_length_token(p.trim()))
            .max_by(|a, b| {
                a.approx_px()
                    .partial_cmp(&b.approx_px())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(Length::Zero);
    }

    let mut input = ParserInput::new(value);
    let mut parser = CssParser::new(&mut input);
    match parser.next() {
        Ok(Token::Dimension { value, unit, .. }) => match unit.as_ref().to_lowercase().as_str() {
            "px" => Length::px(*value),
            "em" => Length::px(*value * ROOT_FONT_SIZE),
            "rem" => Length::px(*value * ROOT_FONT_SIZE),
            "vw" => Length::Vw(*value),
            "vh" => Length::Vh(*value),
            "vmin" => Length::Vw(*value),
            "vmax" => Length::Vh(*value),
            "%" => Length::Percent(*value / 100.0),
            _ => Length::px(*value),
        },
        Ok(Token::Percentage { unit_value, .. }) => Length::Percent(*unit_value),
        Ok(Token::Number { value, .. }) => Length::px(*value),
        Ok(Token::Ident(ident)) if ident.eq_ignore_ascii_case("auto") => Length::Auto,
        _ => Length::Zero,
    }
}

fn parse_line_height(value: &str) -> LineHeight {
    let value = value.trim();
    if value.eq_ignore_ascii_case("normal") {
        return LineHeight::Normal;
    }
    let mut input = ParserInput::new(value);
    let mut parser = CssParser::new(&mut input);
    match parser.next() {
        Ok(Token::Number { value, .. }) => LineHeight::Number(*value),
        Ok(Token::Dimension { value, unit, .. }) if unit.as_ref().eq_ignore_ascii_case("px") => {
            LineHeight::Px(*value)
        }
        Ok(Token::Dimension { value, unit, .. }) if unit.as_ref().eq_ignore_ascii_case("em") => {
            LineHeight::Number(*value)
        }
        Ok(Token::Percentage { unit_value, .. }) => LineHeight::Number(*unit_value),
        _ => LineHeight::Normal,
    }
}

fn parse_clamp(inner: &str) -> Length {
    let parts: Vec<&str> = inner.split(',').collect();
    if parts.len() != 3 {
        return Length::Zero;
    }
    let min = parse_length_token(parts[0].trim());
    let pref = parse_length_token(parts[1].trim());
    let max = parse_length_token(parts[2].trim());
    let preferred_vw = match pref {
        Length::Vw(v) => v,
        _ => 0.0,
    };
    Length::Clamp {
        min_px: length_to_clamp_px(min),
        preferred_px: length_to_clamp_px(pref),
        preferred_vw,
        max_px: length_to_clamp_px(max),
    }
}

fn parse_simple_px(value: &str) -> Option<f32> {
    let mut input = ParserInput::new(value);
    let mut parser = CssParser::new(&mut input);
    match parser.next() {
        Ok(Token::Dimension { value, unit, .. }) if unit.as_ref().eq_ignore_ascii_case("px") => {
            Some(*value)
        }
        Ok(Token::Number { value, .. }) => Some(*value),
        _ => None,
    }
}

fn parse_border_radius(value: &str) -> Option<f32> {
    if let Some(px) = parse_simple_px(value) {
        return Some(px);
    }

    let first = value.split_whitespace().next()?.trim();
    let percent = first.strip_suffix('%')?.parse::<f32>().ok()?;
    Some(if percent <= 0.0 { 0.0 } else { 9999.0 })
}

fn parse_color(value: &str) -> Option<Color> {
    let c = csscolorparser::parse(value).ok()?;
    Some(Color {
        r: (c.r * 255.0).round() as u8,
        g: (c.g * 255.0).round() as u8,
        b: (c.b * 255.0).round() as u8,
        a: (c.a * 255.0).round() as u8,
    })
}

fn parse_background(value: &str) -> Option<Background> {
    let value = value.trim();

    let layers = split_top_level_commas(value);
    if layers.len() > 1 {
        let parsed = layers
            .iter()
            .filter_map(|layer| parse_single_background(layer))
            .collect::<Vec<_>>();
        if !parsed.is_empty() {
            return Some(Background::Layers(parsed));
        }
    }

    parse_single_background(value)
}

struct BackgroundShorthandResult {
    color: Option<Background>,
    image_src: Option<String>,
    repeat_x: bool,
    repeat_y: bool,
    position_x: BackgroundPositionAxis,
    position_y: BackgroundPositionAxis,
}

fn parse_background_image_url(value: &str) -> Option<String> {
    if value.trim().eq_ignore_ascii_case("none") {
        return None;
    }
    let inner = extract_css_function(value.trim(), "url")?;
    let src = inner.trim().trim_matches(|ch| ch == '"' || ch == '\'');
    if src.is_empty() {
        None
    } else {
        Some(src.to_string())
    }
}

fn parse_background_repeat(value: &str) -> Option<(bool, bool)> {
    match value.trim().to_ascii_lowercase().as_str() {
        "repeat" => Some((true, true)),
        "repeat-x" => Some((true, false)),
        "repeat-y" => Some((false, true)),
        "no-repeat" => Some((false, false)),
        _ => None,
    }
}

fn parse_background_position(
    value: &str,
) -> Option<(BackgroundPositionAxis, BackgroundPositionAxis)> {
    let tokens: Vec<&str> = value.split_whitespace().collect();
    parse_position_tokens(&tokens)
}

fn parse_background_shorthand(value: &str) -> Option<BackgroundShorthandResult> {
    if value.contains(',') || contains_top_level_slash(value) {
        return None;
    }

    let mut color = None;
    let mut image_src = None;
    let mut repeat_x = true;
    let mut repeat_y = true;
    let mut position_tokens: Vec<String> = Vec::new();
    let mut saw_repeat = false;

    for token in split_css_tokens(value) {
        if token.starts_with("url(") {
            image_src = parse_background_image_url(&token);
            continue;
        }

        if token.eq_ignore_ascii_case("scroll")
            || token.eq_ignore_ascii_case("fixed")
            || token.eq_ignore_ascii_case("local")
            || token.eq_ignore_ascii_case("cover")
            || token.eq_ignore_ascii_case("contain")
            || token.eq_ignore_ascii_case("border-box")
            || token.eq_ignore_ascii_case("padding-box")
            || token.eq_ignore_ascii_case("content-box")
        {
            return None;
        }

        if color.is_none() {
            if let Some(bg) = parse_background(&token) {
                color = Some(bg);
                continue;
            }
        }

        if !saw_repeat {
            if let Some((rx, ry)) = parse_background_repeat(&token) {
                repeat_x = rx;
                repeat_y = ry;
                saw_repeat = true;
                continue;
            }
        }

        position_tokens.push(token);
    }

    let (position_x, position_y) = if position_tokens.is_empty() {
        (
            BackgroundPositionAxis::Start(Length::Zero),
            BackgroundPositionAxis::Start(Length::Zero),
        )
    } else {
        let refs = position_tokens
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        parse_position_tokens(&refs)?
    };

    Some(BackgroundShorthandResult {
        color,
        image_src,
        repeat_x,
        repeat_y,
        position_x,
        position_y,
    })
}

fn contains_top_level_slash(value: &str) -> bool {
    let mut depth = 0usize;
    for ch in value.chars() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            '/' if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

fn parse_position_tokens(
    tokens: &[&str],
) -> Option<(BackgroundPositionAxis, BackgroundPositionAxis)> {
    match tokens {
        [] => Some((
            BackgroundPositionAxis::Start(Length::Zero),
            BackgroundPositionAxis::Start(Length::Zero),
        )),
        [one] => parse_horizontal_pos(one)
            .map(|x| (x, BackgroundPositionAxis::Center(Length::Zero)))
            .or_else(|| {
                parse_vertical_pos(one).map(|y| (BackgroundPositionAxis::Center(Length::Zero), y))
            })
            .or_else(|| {
                parse_pos_length(one).map(|pos| (pos, BackgroundPositionAxis::Center(Length::Zero)))
            }),
        [first, second] => {
            let x = parse_horizontal_pos(first).or_else(|| parse_pos_length(first))?;
            let y = parse_vertical_pos(second).or_else(|| parse_pos_length(second))?;
            Some((x, y))
        }
        _ => None,
    }
}

fn parse_horizontal_pos(token: &str) -> Option<BackgroundPositionAxis> {
    match token.to_ascii_lowercase().as_str() {
        "left" => Some(BackgroundPositionAxis::Start(Length::Zero)),
        "center" => Some(BackgroundPositionAxis::Center(Length::Zero)),
        "right" => Some(BackgroundPositionAxis::End(Length::Zero)),
        _ => None,
    }
}

fn parse_vertical_pos(token: &str) -> Option<BackgroundPositionAxis> {
    match token.to_ascii_lowercase().as_str() {
        "top" => Some(BackgroundPositionAxis::Start(Length::Zero)),
        "center" => Some(BackgroundPositionAxis::Center(Length::Zero)),
        "bottom" => Some(BackgroundPositionAxis::End(Length::Zero)),
        _ => None,
    }
}

fn parse_pos_length(token: &str) -> Option<BackgroundPositionAxis> {
    if token == "0" || token == "0px" || token == "0%" {
        return Some(BackgroundPositionAxis::Start(Length::Zero));
    }
    let trimmed = token.trim();
    if let Some(v) = trimmed.strip_suffix('%') {
        let pct = v.trim().parse::<f32>().ok()?;
        return Some(BackgroundPositionAxis::Start(Length::Percent(pct / 100.0)));
    }
    if let Some(v) = trimmed.strip_suffix("px") {
        let px = v.trim().parse::<f32>().ok()?;
        return Some(BackgroundPositionAxis::Start(Length::Px(px)));
    }
    if let Ok(px) = trimmed.parse::<f32>() {
        return Some(BackgroundPositionAxis::Start(Length::Px(px)));
    }
    None
}

fn parse_single_background(value: &str) -> Option<Background> {
    let value = value.trim();

    if let Some(inner) = extract_css_function(value, "repeating-linear-gradient") {
        if let Some(bg) = parse_repeating_linear_gradient(&inner) {
            return Some(bg);
        }
    }

    if let Some(inner) = extract_css_function(value, "radial-gradient") {
        let (center_x, center_y, stops) = parse_radial_gradient(&inner);
        if !stops.is_empty() {
            return Some(Background::Radial {
                center_x,
                center_y,
                stops,
            });
        }
    }

    if let Some(inner) = extract_css_function(value, "linear-gradient") {
        let (angle_deg, stops) = parse_linear_gradient(&inner);
        if !stops.is_empty() {
            return Some(Background::Linear { angle_deg, stops });
        }
    }

    parse_color(value).map(Background::Solid)
}

fn parse_repeating_linear_gradient(inner: &str) -> Option<Background> {
    let parts = split_top_level_commas(inner);
    let angle_deg = parts
        .first()
        .and_then(|part| parse_angle_deg(part.trim()))
        .unwrap_or(180.0);
    let color = parts.iter().find_map(|part| {
        let color_text = part
            .split_whitespace()
            .take_while(|token| !token.ends_with("px") && !token.ends_with('%'))
            .collect::<Vec<_>>()
            .join(" ");
        parse_color(color_text.trim())
    })?;

    let mut px_values = parts
        .iter()
        .flat_map(|part| part.split_whitespace())
        .filter_map(|token| token.strip_suffix("px"))
        .filter_map(|value| value.parse::<f32>().ok());

    let stripe_width = px_values.next().unwrap_or(2.0).max(1.0);
    let period = px_values.last().unwrap_or(14.0).max(stripe_width + 1.0);

    Some(Background::RepeatingLinear {
        angle_deg,
        color,
        stripe_width,
        period,
    })
}

fn parse_linear_gradient(inner: &str) -> (f32, Vec<(Color, f32)>) {
    let parts = split_top_level_commas(inner);
    let angle_deg = parts
        .first()
        .and_then(|part| parse_angle_deg(part.trim()))
        .unwrap_or(180.0);
    (angle_deg, parse_gradient_stops(inner))
}

fn parse_radial_gradient(inner: &str) -> (f32, f32, Vec<(Color, f32)>) {
    let parts = split_top_level_commas(inner);
    let mut center_x = 0.5;
    let mut center_y = 0.5;

    if let Some(first) = parts.first() {
        if let Some((_, after_at)) = first.split_once(" at ") {
            let coords = after_at.split_whitespace().collect::<Vec<_>>();
            if let Some(x) = coords.first().and_then(|v| parse_percent(v)) {
                center_x = x;
            }
            if let Some(y) = coords.get(1).and_then(|v| parse_percent(v)) {
                center_y = y;
            }
        }
    }

    (center_x, center_y, parse_gradient_stops(inner))
}

fn parse_angle_deg(value: &str) -> Option<f32> {
    value.strip_suffix("deg")?.trim().parse::<f32>().ok()
}

fn parse_percent(value: &str) -> Option<f32> {
    value
        .strip_suffix('%')?
        .trim()
        .parse::<f32>()
        .ok()
        .map(|v| v / 100.0)
}

fn parse_gradient_stops(inner: &str) -> Vec<(Color, f32)> {
    let parts = split_top_level_commas(inner);
    let mut stops: Vec<(Color, Option<f32>)> = Vec::new();
    let mut last_pos = 0.0f32;

    for (i, part) in parts.iter().enumerate() {
        let part = part.trim();

        if i == 0
            && (part.starts_with("circle")
                || part.starts_with("ellipse")
                || part.contains("deg")
                || part.contains("turn")
                || part.contains("rad"))
        {
            continue;
        }

        let mut pos = None;
        let color_part = if let Some(px_pos) = part.rfind('%') {
            let num_str = part[..px_pos].split_whitespace().last().unwrap_or("0");
            if let Ok(p) = num_str.parse::<f32>() {
                pos = Some(p / 100.0);
            }
            part.rsplit_once(|c: char| c.is_whitespace())
                .map(|(c, _)| c.trim())
                .unwrap_or(part)
        } else if let Some(px_idx) = part.rfind("px") {
            let num_str = part[..px_idx].split_whitespace().last().unwrap_or("0");
            if let Ok(p) = num_str.parse::<f32>() {
                pos = Some(p / 1000.0);
            }
            part.rsplit_once(|c: char| c.is_whitespace())
                .map(|(c, _)| c.trim())
                .unwrap_or(part)
        } else {
            part
        };

        if let Some(c) = parse_color(color_part) {
            if let Some(pos) = pos {
                last_pos = pos;
            }
            stops.push((c, pos));
        }
    }

    if stops.is_empty() {
        return Vec::new();
    }

    let last = stops.len() - 1;
    if stops[0].1.is_none() {
        stops[0].1 = Some(0.0);
    }
    if stops[last].1.is_none() {
        stops[last].1 = Some(1.0);
    }

    let mut result = Vec::with_capacity(stops.len());
    for (idx, (color, pos)) in stops.into_iter().enumerate() {
        let resolved = pos.unwrap_or_else(|| {
            if idx == 0 {
                0.0
            } else if idx == last {
                1.0
            } else {
                last_pos
            }
        });
        last_pos = resolved;
        result.push((color, resolved.clamp(0.0, 1.0)));
    }

    result
}

fn split_top_level_commas(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;

    for (idx, ch) in text.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(text[start..idx].trim());
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }

    if start <= text.len() {
        parts.push(text[start..].trim());
    }

    parts
}

fn extract_css_function(text: &str, name: &str) -> Option<String> {
    let prefix = format!("{}(", name);
    let start = text.find(&prefix)?;
    let after = start + prefix.len();
    let rest = &text[after..];
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
    if depth == 0 {
        Some(rest[..end].to_string())
    } else {
        None
    }
}

fn parse_font_weight(value: &str) -> u32 {
    match value.trim() {
        "bold" => 700,
        "normal" => 400,
        "lighter" => 300,
        "bolder" => 600,
        n => n.parse().unwrap_or(400),
    }
}

fn parse_display(value: &str) -> Option<Display> {
    match value.trim() {
        "block" => Some(Display::Block),
        "inline" => Some(Display::Inline),
        "flex" => Some(Display::Flex),
        "grid" => Some(Display::Grid),
        "inline-flex" => Some(Display::InlineFlex),
        "inline-grid" => Some(Display::InlineGrid),
        "none" => Some(Display::None),
        _ => None,
    }
}

fn parse_flex_direction(value: &str) -> FlexDirection {
    match value.trim() {
        "row" => FlexDirection::Row,
        "column" => FlexDirection::Column,
        "row-reverse" => FlexDirection::RowReverse,
        "column-reverse" => FlexDirection::ColumnReverse,
        _ => FlexDirection::Unspecified,
    }
}

fn parse_justify_content(value: &str) -> JustifyContent {
    match value.trim() {
        "flex-start" | "start" => JustifyContent::FlexStart,
        "center" => JustifyContent::Center,
        "flex-end" | "end" => JustifyContent::FlexEnd,
        "space-between" => JustifyContent::SpaceBetween,
        "space-around" => JustifyContent::SpaceAround,
        _ => JustifyContent::Unspecified,
    }
}

fn parse_align_items(value: &str) -> AlignItems {
    match value.trim() {
        "flex-start" | "start" => AlignItems::FlexStart,
        "center" => AlignItems::Center,
        "flex-end" | "end" => AlignItems::FlexEnd,
        "stretch" => AlignItems::Stretch,
        _ => AlignItems::Unspecified,
    }
}

fn parse_flex_wrap(value: &str) -> FlexWrap {
    match value.trim() {
        "wrap" => FlexWrap::Wrap,
        "nowrap" => FlexWrap::NoWrap,
        _ => FlexWrap::Unspecified,
    }
}

fn parse_flex_shorthand(style: &mut ComputedStyle, value: &str) {
    let value = value.trim();
    match value {
        "auto" => {
            style.flex_grow = 1.0;
            style.flex_shrink = 1.0;
            style.flex_basis = Length::Auto;
            return;
        }
        "none" => {
            style.flex_grow = 0.0;
            style.flex_shrink = 0.0;
            style.flex_basis = Length::Auto;
            return;
        }
        _ => {}
    }

    let parts: Vec<&str> = value.split_whitespace().collect();
    if parts.is_empty() {
        return;
    }

    if parts.len() == 1 {
        if let Ok(grow) = parts[0].parse::<f32>() {
            style.flex_grow = grow.max(0.0);
            style.flex_shrink = 1.0;
            style.flex_basis = Length::Zero;
        } else {
            style.flex_basis = parse_length_token(parts[0]);
        }
        return;
    }

    if let Ok(grow) = parts[0].parse::<f32>() {
        style.flex_grow = grow.max(0.0);
    }

    if parts.len() == 2 {
        if let Ok(shrink) = parts[1].parse::<f32>() {
            style.flex_shrink = shrink.max(0.0);
        } else {
            style.flex_shrink = 1.0;
            style.flex_basis = parse_length_token(parts[1]);
        }
        return;
    }

    if let Ok(shrink) = parts[1].parse::<f32>() {
        style.flex_shrink = shrink.max(0.0);
    }
    if let Some(basis) = parts.get(2) {
        style.flex_basis = parse_length_token(basis);
    }
}

fn parse_border_shorthand(value: &str) -> (f32, Option<Color>) {
    let mut width = 0.0;
    let mut color = None;
    for part in split_css_tokens(value) {
        if let Some(v) = parse_simple_px(&part) {
            width = v;
        } else if let Some(c) = parse_color(&part) {
            color = Some(c);
        }
    }
    if width == 0.0 && color.is_none() {
        match value.trim() {
            "none" => (0.0, None),
            _ => (3.0, Some(Color::BLACK)),
        }
    } else if width == 0.0 {
        (3.0, color)
    } else {
        (width, color)
    }
}

fn parse_box_shadow(value: &str) -> Option<BoxShadow> {
    let value = value.trim();
    if value == "none" || value.is_empty() {
        return None;
    }

    let tokens = split_css_tokens(value);
    let mut numbers: Vec<f32> = Vec::new();
    let mut color: Option<Color> = None;

    for token in &tokens {
        if let Some(c) = parse_color(token) {
            color = Some(c);
        } else if let Some(n) = token.trim_end_matches("px").parse::<f32>().ok() {
            numbers.push(n);
        }
    }

    if numbers.is_empty() && color.is_none() {
        return None;
    }

    Some(BoxShadow {
        offset_x: numbers.get(0).copied().unwrap_or(0.0),
        offset_y: numbers.get(1).copied().unwrap_or(0.0),
        blur: numbers.get(2).copied().unwrap_or(0.0),
        spread: numbers.get(3).copied().unwrap_or(0.0),
        color: color.unwrap_or(Color {
            r: 0,
            g: 0,
            b: 0,
            a: 51,
        }),
    })
}

fn split_css_tokens(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut paren_depth = 0;

    for ch in value.chars() {
        match ch {
            '(' => {
                paren_depth += 1;
                current.push(ch);
            }
            ')' => {
                paren_depth -= 1;
                current.push(ch);
            }
            ' ' | '\t' if paren_depth == 0 => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn parse_margin_shorthand(value: &str) -> (Length, Length, Length, Length) {
    let parts: Vec<&str> = value.split_whitespace().collect();
    let parse = |s: &str| -> Length {
        if s.eq_ignore_ascii_case("auto") {
            Length::Auto
        } else {
            parse_length_token(s)
        }
    };
    match parts.len() {
        1 => {
            let v = parse(parts[0]);
            (v, v, v, v)
        }
        2 => {
            let v = parse(parts[0]);
            let h = parse(parts[1]);
            (v, v, h, h)
        }
        3 => {
            let t = parse(parts[0]);
            let h = parse(parts[1]);
            let b = parse(parts[2]);
            (t, b, h, h)
        }
        4 => {
            let t = parse(parts[0]);
            let r = parse(parts[1]);
            let b = parse(parts[2]);
            let l = parse(parts[3]);
            (t, b, l, r)
        }
        _ => (Length::Zero, Length::Zero, Length::Zero, Length::Zero),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flex_shorthand_two_values_can_set_basis() {
        let mut style = default_block();
        parse_flex_shorthand(&mut style, "1 20px");

        assert_eq!(style.flex_grow, 1.0);
        assert_eq!(style.flex_shrink, 1.0);
        assert_eq!(style.flex_basis, Length::Px(20.0));
    }

    #[test]
    fn flex_shorthand_three_values_sets_grow_shrink_basis() {
        let mut style = default_block();
        parse_flex_shorthand(&mut style, "0 0 16rem");

        assert_eq!(style.flex_grow, 0.0);
        assert_eq!(style.flex_shrink, 0.0);
        assert_eq!(style.flex_basis.approx_px(), 256.0);
    }

    #[test]
    fn visibility_hidden_does_not_change_display() {
        let mut style = default_block();
        let decl = Declaration {
            property: "visibility".to_string(),
            value: "hidden".to_string(),
            important: false,
        };

        apply_declaration(&mut style, &decl);

        assert_eq!(style.display, Display::Block);
        assert_eq!(style.visibility, Visibility::Hidden);
    }
}
