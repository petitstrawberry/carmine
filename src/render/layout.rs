use taffy::geometry::{Rect, Size};
use taffy::style::{
    AvailableSpace, Display as TaffyDisplay, FlexDirection, LengthPercentage, LengthPercentageAuto,
    Style as TaffyStyle,
};
use taffy::{NodeId, TaffyTree};

use crate::render::style::{
    AlignItems, ComputedStyle, Display as StyleDisplay, FlexDirection as StyleFlexDir,
    FlexWrap as StyleFlexWrap, JustifyContent, Length, StyledKind, StyledNode,
};

pub struct LayoutNode {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub tag: Option<String>,
    pub style: ComputedStyle,
    pub pseudo_before: Option<ComputedStyle>,
    pub pseudo_after: Option<ComputedStyle>,
    pub text: Option<String>,
    pub children: Vec<LayoutNode>,
}

struct TextMeasureData {
    text: String,
    font_size: f32,
}

struct TaffyLink<'a> {
    styled: &'a StyledNode,
    taffy_id: NodeId,
    children: Vec<TaffyLink<'a>>,
    text: Option<String>,
}

pub fn layout(root: &StyledNode, viewport_w: u32, viewport_h: u32) -> LayoutNode {
    let mut tree: TaffyTree<TextMeasureData> = TaffyTree::new();
    let vw = viewport_w as f32;
    let vh = viewport_h as f32;
    let link = build_taffy_tree(root, &mut tree, vw, vh);

    let available = Size {
        width: AvailableSpace::Definite(vw),
        height: AvailableSpace::Definite(vh),
    };

    tree.compute_layout_with_measure(
        link.taffy_id,
        available,
        |known, avail, _id, context, _style| match context {
            Some(data) => measure_text(&data.text, data.font_size, known, avail),
            None => Size::ZERO,
        },
    )
    .expect("taffy layout failed");

    extract(&link, &tree, 0.0, 0.0)
}

fn build_taffy_tree<'a>(
    node: &'a StyledNode,
    tree: &mut TaffyTree<TextMeasureData>,
    vw: f32,
    vh: f32,
) -> TaffyLink<'a> {
    if node.style.display == StyleDisplay::None {
        let id = tree
            .new_leaf(TaffyStyle {
                display: TaffyDisplay::None,
                ..Default::default()
            })
            .expect("taffy new_leaf");
        return TaffyLink {
            styled: node,
            taffy_id: id,
            children: vec![],
            text: None,
        };
    }

    let visible_children: Vec<&StyledNode> = node
        .children
        .iter()
        .filter(|c| {
            if c.style.display == StyleDisplay::None {
                return false;
            }
            if let StyledKind::Text(t) = &c.kind {
                return !t.trim().is_empty();
            }
            true
        })
        .collect();

    let has_block_children = visible_children.iter().any(|c| {
        matches!(
            c.style.display,
            StyleDisplay::Block | StyleDisplay::Flex | StyleDisplay::Grid
        )
    });

    if !has_block_children && !visible_children.is_empty() {
        let text = collapse_html_whitespace(&collect_inline_text(node));
        if !text.trim().is_empty() {
            let ctx = TextMeasureData {
                text: text.clone(),
                font_size: node.style.font_size,
            };
            let id = tree
                .new_leaf_with_context(to_taffy_style_for_node(node, vw, vh), ctx)
                .expect("taffy new_leaf_with_context");
            return TaffyLink {
                styled: node,
                taffy_id: id,
                children: vec![],
                text: Some(text),
            };
        }
    }

    if visible_children.is_empty() {
        let id = match &node.kind {
            StyledKind::Text(text) => {
                let text = collapse_html_whitespace(text);
                let ctx = TextMeasureData {
                    text,
                    font_size: node.style.font_size,
                };
                tree.new_leaf_with_context(to_taffy_style_for_node(node, vw, vh), ctx)
                    .expect("taffy new_leaf_with_context")
            }
            _ => tree
                .new_leaf(to_taffy_style_for_node(node, vw, vh))
                .expect("taffy new_leaf"),
        };
        TaffyLink {
            styled: node,
            taffy_id: id,
            children: vec![],
            text: None,
        }
    } else {
        let children: Vec<TaffyLink> = visible_children
            .iter()
            .map(|c| build_taffy_tree(c, tree, vw, vh))
            .collect();
        let child_ids: Vec<NodeId> = children.iter().map(|c| c.taffy_id).collect();
        let id = tree
            .new_with_children(to_taffy_style_for_node(node, vw, vh), &child_ids)
            .expect("taffy new_with_children");
        TaffyLink {
            styled: node,
            taffy_id: id,
            children,
            text: None,
        }
    }
}

fn collect_inline_text(node: &StyledNode) -> String {
    match &node.kind {
        StyledKind::Text(t) => t.clone(),
        StyledKind::Element { .. } | StyledKind::Document => node
            .children
            .iter()
            .filter(|c| c.style.display != StyleDisplay::None)
            .map(collect_inline_text)
            .collect::<Vec<_>>()
            .join(""),
    }
}

fn collapse_html_whitespace(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut previous_was_space = true;

    for ch in text.chars() {
        if ch.is_whitespace() {
            if !previous_was_space {
                result.push(' ');
            }
            previous_was_space = true;
        } else {
            result.push(ch);
            previous_was_space = false;
        }
    }

    if result.ends_with(' ') {
        result.pop();
    }
    result
}

fn extract(
    link: &TaffyLink,
    tree: &TaffyTree<TextMeasureData>,
    parent_x: f32,
    parent_y: f32,
) -> LayoutNode {
    let layout = tree.layout(link.taffy_id).expect("taffy layout result");
    let x = parent_x + layout.location.x;
    let y = parent_y + layout.location.y;

    let text = if let Some(ref t) = link.text {
        Some(t.clone())
    } else {
        match &link.styled.kind {
            StyledKind::Text(t) => Some(collapse_html_whitespace(t)),
            _ => None,
        }
    };

    let tag = match &link.styled.kind {
        StyledKind::Element { tag } => Some(tag.clone()),
        _ => None,
    };

    let children = link
        .children
        .iter()
        .map(|c| extract(c, tree, x, y))
        .collect();

    LayoutNode {
        x,
        y,
        width: layout.size.width,
        height: layout.size.height,
        tag,
        style: link.styled.style.clone(),
        pseudo_before: link.styled.pseudo_before.clone(),
        pseudo_after: link.styled.pseudo_after.clone(),
        text,
        children,
    }
}

fn to_taffy_style(s: &ComputedStyle, vw: f32, vh: f32) -> TaffyStyle {
    let display = match s.display {
        StyleDisplay::None => TaffyDisplay::None,
        StyleDisplay::Flex => TaffyDisplay::Flex,
        StyleDisplay::Grid => {
            if s.grid_template_columns.is_empty() {
                TaffyDisplay::Flex
            } else {
                TaffyDisplay::Grid
            }
        }
        StyleDisplay::Block | StyleDisplay::Inline => TaffyDisplay::Flex,
    };

    let flex_direction = match s.flex_direction {
        StyleFlexDir::Row => FlexDirection::Row,
        StyleFlexDir::Column => FlexDirection::Column,
        StyleFlexDir::RowReverse => FlexDirection::RowReverse,
        StyleFlexDir::ColumnReverse => FlexDirection::ColumnReverse,
        StyleFlexDir::Unspecified => {
            if s.display == StyleDisplay::Flex {
                FlexDirection::Row
            } else {
                FlexDirection::Column
            }
        }
    };

    let justify_content = match s.justify_content {
        JustifyContent::FlexStart => taffy::style::JustifyContent::FlexStart,
        JustifyContent::Center => taffy::style::JustifyContent::Center,
        JustifyContent::FlexEnd => taffy::style::JustifyContent::FlexEnd,
        JustifyContent::SpaceBetween => taffy::style::JustifyContent::SpaceBetween,
        JustifyContent::SpaceAround => taffy::style::JustifyContent::SpaceAround,
        JustifyContent::Unspecified => taffy::style::JustifyContent::FlexStart,
    };

    let align_items = match s.align_items {
        AlignItems::FlexStart => taffy::style::AlignItems::FlexStart,
        AlignItems::Center => taffy::style::AlignItems::Center,
        AlignItems::FlexEnd => taffy::style::AlignItems::FlexEnd,
        AlignItems::Stretch => taffy::style::AlignItems::Stretch,
        AlignItems::Unspecified => taffy::style::AlignItems::Stretch,
    };

    let flex_wrap = match s.flex_wrap {
        StyleFlexWrap::Wrap => taffy::style::FlexWrap::Wrap,
        StyleFlexWrap::NoWrap | StyleFlexWrap::Unspecified => taffy::style::FlexWrap::NoWrap,
    };

    TaffyStyle {
        display,
        flex_direction,
        flex_wrap,
        justify_content: Some(justify_content),
        align_items: Some(align_items),
        gap: Size {
            width: length_to_lp(s.gap, vw, vh),
            height: length_to_lp(s.gap, vw, vh),
        },
        size: Size {
            width: length_to_dim(s.width, vw, vh),
            height: length_to_dim(s.height, vw, vh),
        },
        max_size: Size {
            width: length_to_dim(s.max_width, vw, vh),
            height: taffy::style::Dimension::Auto,
        },
        min_size: Size {
            width: taffy::style::Dimension::Auto,
            height: length_to_dim(s.min_height, vw, vh),
        },
        box_sizing: match s.box_sizing {
            crate::render::style::BoxSizing::BorderBox => taffy::style::BoxSizing::BorderBox,
            crate::render::style::BoxSizing::ContentBox => taffy::style::BoxSizing::ContentBox,
        },
        grid_template_columns: parse_grid_tracks(&s.grid_template_columns, vw, vh),
        margin: Rect {
            top: length_to_lpa(s.margin_top, vw, vh),
            bottom: length_to_lpa(s.margin_bottom, vw, vh),
            left: length_to_lpa(s.margin_left, vw, vh),
            right: length_to_lpa(s.margin_right, vw, vh),
        },
        padding: Rect {
            top: length_to_lp(s.padding_top, vw, vh),
            bottom: length_to_lp(s.padding_bottom, vw, vh),
            left: length_to_lp(s.padding_left, vw, vh),
            right: length_to_lp(s.padding_right, vw, vh),
        },
        align_content: Some(taffy::style::AlignContent::Stretch),
        ..Default::default()
    }
}

fn to_taffy_style_for_node(node: &StyledNode, vw: f32, vh: f32) -> TaffyStyle {
    let mut style = to_taffy_style(&node.style, vw, vh);
    if matches!(node.kind, StyledKind::Document) {
        style.size.width = taffy::style::Dimension::Length(vw);
        style.size.height = taffy::style::Dimension::Length(vh);
    }
    style
}

fn parse_grid_tracks(text: &str, vw: f32, vh: f32) -> Vec<taffy::style::TrackSizingFunction> {
    use taffy::style::{GridTrackRepetition, TrackSizingFunction};

    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }

    if let Some(inner) = extract_function(text, "repeat") {
        let parts: Vec<&str> = inner.splitn(2, ',').collect();
        if parts.len() != 2 {
            return Vec::new();
        }
        let count = match parts[0].trim() {
            "auto-fit" => GridTrackRepetition::AutoFit,
            "auto-fill" => GridTrackRepetition::AutoFill,
            n => match n.parse::<u16>() {
                Ok(num) => GridTrackRepetition::Count(num),
                Err(_) => return Vec::new(),
            },
        };
        let track_str = parts[1].trim();
        let tracks = parse_single_track_list(track_str, vw, vh);
        if tracks.is_empty() {
            return Vec::new();
        }
        return vec![TrackSizingFunction::Repeat(count, tracks)];
    }

    parse_single_track_list(text, vw, vh)
        .into_iter()
        .map(TrackSizingFunction::Single)
        .collect()
}

fn parse_single_track_list(
    text: &str,
    vw: f32,
    vh: f32,
) -> Vec<taffy::style::NonRepeatedTrackSizingFunction> {
    use taffy::style::{
        MaxTrackSizingFunction, MinTrackSizingFunction, NonRepeatedTrackSizingFunction,
    };

    let mut result = Vec::new();

    for part in split_track_list(text) {
        if let Some(inner) = extract_function(part, "minmax") {
            let minmax: Vec<&str> = inner.splitn(2, ',').collect();
            if minmax.len() != 2 {
                continue;
            }
            let min = parse_min_track(minmax[0].trim(), vw, vh);
            let max = parse_max_track(minmax[1].trim(), vw, vh);
            result.push(NonRepeatedTrackSizingFunction { min, max });
        } else {
            let max = parse_max_track(part, vw, vh);
            let min = match &max {
                MaxTrackSizingFunction::Fixed(_) => MinTrackSizingFunction::Auto,
                _ => MinTrackSizingFunction::Auto,
            };
            result.push(NonRepeatedTrackSizingFunction { min, max });
        }
    }

    result
}

fn parse_min_track(s: &str, vw: f32, vh: f32) -> taffy::style::MinTrackSizingFunction {
    use taffy::style::MinTrackSizingFunction;
    match s {
        "auto" => MinTrackSizingFunction::Auto,
        "min-content" => MinTrackSizingFunction::MinContent,
        "max-content" => MinTrackSizingFunction::MaxContent,
        _ => {
            if let Some(lp) = parse_lp_from_str(s, vw, vh) {
                MinTrackSizingFunction::Fixed(lp)
            } else {
                MinTrackSizingFunction::Auto
            }
        }
    }
}

fn parse_max_track(s: &str, vw: f32, vh: f32) -> taffy::style::MaxTrackSizingFunction {
    use taffy::style::MaxTrackSizingFunction;
    if let Some(fr) = s.strip_suffix("fr") {
        if let Ok(v) = fr.trim().parse::<f32>() {
            return MaxTrackSizingFunction::Fraction(v);
        }
    }
    match s {
        "auto" => MaxTrackSizingFunction::Auto,
        "min-content" => MaxTrackSizingFunction::MinContent,
        "max-content" => MaxTrackSizingFunction::MaxContent,
        _ => {
            if let Some(lp) = parse_lp_from_str(s, vw, vh) {
                MaxTrackSizingFunction::Fixed(lp)
            } else {
                MaxTrackSizingFunction::Auto
            }
        }
    }
}

fn parse_lp_from_str(s: &str, vw: f32, vh: f32) -> Option<LengthPercentage> {
    let len = crate::render::style::Length::parse(s);
    Some(match len {
        crate::render::style::Length::Px(v) => LengthPercentage::Length(v),
        crate::render::style::Length::Percent(v) => LengthPercentage::Percent(v),
        crate::render::style::Length::Vw(v) => LengthPercentage::Length(v * vw / 100.0),
        crate::render::style::Length::Vh(v) => LengthPercentage::Length(v * vh / 100.0),
        crate::render::style::Length::Clamp { .. } => {
            LengthPercentage::Length(len.resolve_px(vw, vw))
        }
        _ => return None,
    })
}

fn extract_function(text: &str, name: &str) -> Option<String> {
    let prefix = format!("{}(", name);
    if text.starts_with(&prefix) && text.ends_with(')') {
        Some(text[prefix.len()..text.len() - 1].to_string())
    } else {
        None
    }
}

fn split_track_list(text: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;

    for (idx, ch) in text.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            c if c.is_whitespace() && depth == 0 => {
                if start < idx {
                    result.push(text[start..idx].trim());
                }
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }

    if start < text.len() {
        result.push(text[start..].trim());
    }

    result.into_iter().filter(|part| !part.is_empty()).collect()
}

fn length_to_lp(l: Length, vw: f32, vh: f32) -> LengthPercentage {
    match l {
        Length::Px(v) => LengthPercentage::Length(v),
        Length::Percent(v) => LengthPercentage::Percent(v),
        Length::Vw(v) => LengthPercentage::Length(v * vw / 100.0),
        Length::Vh(v) => LengthPercentage::Length(v * vh / 100.0),
        Length::Clamp { .. } => LengthPercentage::Length(l.resolve_px(vw, vw)),
        Length::Zero => LengthPercentage::Length(0.0),
        _ => LengthPercentage::Length(0.0),
    }
}

fn length_to_lpa(l: Length, vw: f32, vh: f32) -> LengthPercentageAuto {
    match l {
        Length::Px(v) => LengthPercentageAuto::Length(v),
        Length::Percent(v) => LengthPercentageAuto::Percent(v),
        Length::Vw(v) => LengthPercentageAuto::Length(v * vw / 100.0),
        Length::Vh(v) => LengthPercentageAuto::Length(v * vh / 100.0),
        Length::Clamp { .. } => LengthPercentageAuto::Length(l.resolve_px(vw, vw)),
        Length::Auto => LengthPercentageAuto::Auto,
        Length::Zero => LengthPercentageAuto::Length(0.0),
    }
}

fn length_to_dim(l: Length, vw: f32, vh: f32) -> taffy::style::Dimension {
    match l {
        Length::Px(v) => taffy::style::Dimension::Length(v),
        Length::Percent(v) => taffy::style::Dimension::Percent(v),
        Length::Vw(v) => taffy::style::Dimension::Length(v * vw / 100.0),
        Length::Vh(v) => taffy::style::Dimension::Length(v * vh / 100.0),
        Length::Clamp { .. } => taffy::style::Dimension::Length(l.resolve_px(vw, vw)),
        Length::Auto => taffy::style::Dimension::Auto,
        Length::Zero => taffy::style::Dimension::Length(0.0),
    }
}

fn measure_text(
    text: &str,
    font_size: f32,
    known: Size<Option<f32>>,
    available: Size<AvailableSpace>,
) -> Size<f32> {
    let char_width = font_size * 0.55;
    let line_height = font_size * 1.2;

    let avail_width = match available.width {
        AvailableSpace::Definite(w) => w,
        _ => f32::MAX,
    };

    let (natural_width, line_count) = measure_wrapped_text(text, char_width, avail_width);
    let width = known.width.unwrap_or(natural_width.max(char_width));
    let height = known.height.unwrap_or(line_height * line_count as f32);

    Size { width, height }
}

fn measure_wrapped_text(text: &str, char_width: f32, max_width: f32) -> (f32, usize) {
    if text.is_empty() {
        return (0.0, 1);
    }

    if !max_width.is_finite() || max_width <= 0.0 {
        return (text.chars().count() as f32 * char_width, 1);
    }

    let mut line_width = 0.0f32;
    let mut max_line_width = 0.0f32;
    let mut lines = 1usize;
    let space_width = char_width;

    for word in text.split(' ') {
        let word_width = word.chars().count() as f32 * char_width;
        let sep = if line_width > 0.0 { space_width } else { 0.0 };

        if line_width > 0.0 && line_width + sep + word_width > max_width {
            max_line_width = max_line_width.max(line_width);
            lines += 1;
            line_width = word_width;
        } else {
            line_width += sep + word_width;
        }

        while line_width > max_width && max_width > char_width {
            max_line_width = max_line_width.max(max_width);
            lines += 1;
            line_width -= max_width;
        }
    }

    max_line_width = max_line_width.max(line_width).min(max_width);
    (max_line_width, lines)
}
