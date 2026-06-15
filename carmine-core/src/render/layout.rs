use taffy::geometry::{Rect, Size};
use taffy::style::{
    AvailableSpace, Display as TaffyDisplay, FlexDirection, LengthPercentage, LengthPercentageAuto,
    Style as TaffyStyle,
};
use taffy::{NodeId, TaffyTree};

use crate::render::style::{
    AlignItems, BoxSizing, ComputedStyle, Display as StyleDisplay, FlexDirection as StyleFlexDir,
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
    pub image: Option<LayoutImage>,
    pub children: Vec<LayoutNode>,
}

#[derive(Clone, Debug)]
pub struct LayoutImage {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl LayoutNode {
    pub fn content_bottom(&self) -> f32 {
        let self_bottom = self.y + self.height;
        self.children
            .iter()
            .fold(self_bottom, |max, child| max.max(child.content_bottom()))
    }
}

struct TextMeasureData {
    text: String,
    font_size: f32,
    line_height_px: f32,
}

struct TaffyLink<'a> {
    styled: &'a StyledNode,
    taffy_id: NodeId,
    children: Vec<TaffyLink<'a>>,
    text: Option<String>,
}

pub fn layout(root: &StyledNode, viewport_w: u32, viewport_h: u32) -> LayoutNode {
    layout_with_images(root, viewport_w, viewport_h, None)
}

pub fn layout_with_images(
    root: &StyledNode,
    viewport_w: u32,
    viewport_h: u32,
    image_loader: Option<&Box<dyn Fn(&str) -> Option<Vec<u8>> + Send + Sync>>,
) -> LayoutNode {
    let mut tree: TaffyTree<TextMeasureData> = TaffyTree::new();
    let vw = viewport_w as f32;
    let vh = viewport_h as f32;
    let link = build_taffy_tree(root, &mut tree, vw, vh, Some(vw));

    let available = Size {
        width: AvailableSpace::Definite(vw),
        height: AvailableSpace::MaxContent,
    };

    tree.compute_layout_with_measure(
        link.taffy_id,
        available,
        |known, avail, _id, context, _style| match context {
            Some(data) => measure_text(
                &data.text,
                data.font_size,
                data.line_height_px,
                known,
                avail,
            ),
            None => Size::ZERO,
        },
    )
    .expect("taffy layout failed");

    let mut root_layout = extract(&link, &tree, 0.0, 0.0);
    if let Some(ref loader) = image_loader {
        let l: &dyn Fn(&str) -> Option<Vec<u8>> = loader.as_ref();
        inject_images(&mut root_layout, &link, l);
    }
    root_layout
}

fn inject_images(
    node: &mut LayoutNode,
    link: &TaffyLink,
    loader: &dyn Fn(&str) -> Option<Vec<u8>>,
) {
    if let Some(ref src) = link.styled.img_src {
        if let Some(img) = load_image_for_layout(src, loader) {
            if node.width == 0.0 && node.height == 0.0 {
                node.width = img.width as f32;
                node.height = img.height as f32;
            } else if node.width > 0.0 && node.height > 0.0 {
            } else if node.width > 0.0 {
                node.height = node.width * img.height as f32 / img.width as f32;
            } else if node.height > 0.0 {
                node.width = node.height * img.width as f32 / img.height as f32;
            }
            node.image = Some(img);
        }
    }
    for (child_layout, child_link) in node.children.iter_mut().zip(link.children.iter()) {
        inject_images(child_layout, child_link, loader);
    }
}

fn build_taffy_tree<'a>(
    node: &'a StyledNode,
    tree: &mut TaffyTree<TextMeasureData>,
    vw: f32,
    vh: f32,
    containing_content_width: Option<f32>,
) -> TaffyLink<'a> {
    build_taffy_tree_inner(node, tree, vw, vh, containing_content_width, true)
}

fn build_taffy_tree_inner<'a>(
    node: &'a StyledNode,
    tree: &mut TaffyTree<TextMeasureData>,
    vw: f32,
    vh: f32,
    containing_content_width: Option<f32>,
    parent_stretches: bool,
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

    let node_content_width = estimate_content_width(node, containing_content_width, vw, vh);
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

    let has_natural_block_children = visible_children
        .iter()
        .any(|c| !is_naturally_inline_kind(&c.kind));

    let has_block_children = has_natural_block_children;
    let has_element_children = visible_children
        .iter()
        .any(|c| matches!(c.kind, StyledKind::Element { .. }));
    let is_flex_or_grid_container = matches!(
        node.style.display,
        StyleDisplay::Flex
            | StyleDisplay::InlineFlex
            | StyleDisplay::Grid
            | StyleDisplay::InlineGrid
    );
    let should_collapse_inline_text =
        !has_block_children && !has_element_children && !visible_children.is_empty();

    let self_stretches = matches!(
        node.style.align_items,
        crate::render::style::AlignItems::Stretch | crate::render::style::AlignItems::Unspecified
    );

    if should_collapse_inline_text {
        let text = collapse_html_whitespace(&collect_inline_text(node));
        if !text.trim().is_empty() {
            let ctx = TextMeasureData {
                text: text.clone(),
                font_size: node.style.font_size,
                line_height_px: node.style.line_height.resolve_px(node.style.font_size),
            };
            let mut style = to_taffy_style_for_node(node, vw, vh, node_content_width);
            if parent_stretches && matches!(node.style.display, StyleDisplay::Inline) {
                style.align_self = Some(taffy::style::AlignItems::FlexStart);
            }
            let id = tree
                .new_leaf_with_context(style, ctx)
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
        let mut leaf_style = to_taffy_style_for_node(node, vw, vh, node_content_width);
        if parent_stretches && matches!(node.style.display, StyleDisplay::Inline) {
            leaf_style.align_self = Some(taffy::style::AlignItems::FlexStart);
        }
        let id = match &node.kind {
            StyledKind::Text(text) => {
                let text = collapse_html_whitespace(text);
                let ctx = TextMeasureData {
                    text,
                    font_size: node.style.font_size,
                    line_height_px: node.style.line_height.resolve_px(node.style.font_size),
                };
                tree.new_leaf_with_context(leaf_style, ctx)
                    .expect("taffy new_leaf_with_context")
            }
            _ => tree.new_leaf(leaf_style).expect("taffy new_leaf"),
        };
        TaffyLink {
            styled: node,
            taffy_id: id,
            children: vec![],
            text: None,
        }
    } else {
        let needs_inline_grouping = has_block_children
            && visible_children
                .iter()
                .any(|c| is_naturally_inline_kind(&c.kind))
            && matches!(
                node.style.display,
                StyleDisplay::Block | StyleDisplay::Inline
            );

        let children: Vec<TaffyLink> = if needs_inline_grouping {
            group_children_for_block_layout(
                &visible_children,
                tree,
                vw,
                vh,
                node_content_width,
                self_stretches,
            )
        } else {
            visible_children
                .iter()
                .map(|c| {
                    build_taffy_tree_inner(c, tree, vw, vh, node_content_width, self_stretches)
                })
                .collect()
        };

        let mut container_style = to_taffy_style_for_node(node, vw, vh, node_content_width);
        if parent_stretches && matches!(node.style.display, StyleDisplay::Inline) {
            container_style.align_self = Some(taffy::style::AlignItems::FlexStart);
        }
        if !has_block_children
            && !needs_inline_grouping
            && matches!(
                node.style.display,
                StyleDisplay::Block | StyleDisplay::Inline
            )
            && matches!(container_style.flex_direction, FlexDirection::Column)
        {
            container_style.flex_direction = FlexDirection::Row;
            container_style.flex_wrap = taffy::style::FlexWrap::Wrap;
        }

        let child_ids: Vec<NodeId> = children.iter().map(|c| c.taffy_id).collect();
        let id = tree
            .new_with_children(container_style, &child_ids)
            .expect("taffy new_with_children");
        TaffyLink {
            styled: node,
            taffy_id: id,
            children,
            text: None,
        }
    }
}

fn group_children_for_block_layout<'a>(
    children: &[&'a StyledNode],
    tree: &mut TaffyTree<TextMeasureData>,
    vw: f32,
    vh: f32,
    containing_width: Option<f32>,
    parent_stretches: bool,
) -> Vec<TaffyLink<'a>> {
    let mut groups: Vec<TaffyLink<'a>> = Vec::new();
    let mut inline_run: Vec<&StyledNode> = Vec::new();

    for child in children {
        if is_naturally_inline_kind(&child.kind) {
            inline_run.push(child);
        } else {
            if !inline_run.is_empty() {
                let run_links = inline_run
                    .iter()
                    .map(|c| {
                        build_taffy_tree_inner(c, tree, vw, vh, containing_width, parent_stretches)
                    })
                    .collect::<Vec<_>>();
                let run_ids: Vec<NodeId> = run_links.iter().map(|c| c.taffy_id).collect();
                let row_id = tree
                    .new_with_children(
                        TaffyStyle {
                            display: TaffyDisplay::Flex,
                            flex_direction: FlexDirection::Row,
                            flex_wrap: taffy::style::FlexWrap::Wrap,
                            ..Default::default()
                        },
                        &run_ids,
                    )
                    .expect("taffy new_with_children inline group");
                groups.push(TaffyLink {
                    styled: children.first().copied().unwrap_or(child),
                    taffy_id: row_id,
                    children: run_links,
                    text: None,
                });
                inline_run.clear();
            }
            groups.push(build_taffy_tree_inner(
                child,
                tree,
                vw,
                vh,
                containing_width,
                parent_stretches,
            ));
        }
    }

    if !inline_run.is_empty() {
        let run_links = inline_run
            .iter()
            .map(|c| build_taffy_tree_inner(c, tree, vw, vh, containing_width, parent_stretches))
            .collect::<Vec<_>>();
        let run_ids: Vec<NodeId> = run_links.iter().map(|c| c.taffy_id).collect();
        let row_id = tree
            .new_with_children(
                TaffyStyle {
                    display: TaffyDisplay::Flex,
                    flex_direction: FlexDirection::Row,
                    flex_wrap: taffy::style::FlexWrap::Wrap,
                    ..Default::default()
                },
                &run_ids,
            )
            .expect("taffy new_with_children inline group");
        groups.push(TaffyLink {
            styled: children
                .first()
                .copied()
                .unwrap_or(children.last().copied().unwrap()),
            taffy_id: row_id,
            children: run_links,
            text: None,
        });
    }

    groups
}

fn is_naturally_inline_kind(kind: &StyledKind) -> bool {
    match kind {
        StyledKind::Text(_) => true,
        StyledKind::Element { tag } => is_naturally_inline_tag(tag),
        StyledKind::Document => false,
    }
}

fn is_naturally_inline_tag(tag: &str) -> bool {
    matches!(
        tag.to_lowercase().as_str(),
        "a" | "abbr"
            | "b"
            | "bdi"
            | "bdo"
            | "br"
            | "cite"
            | "code"
            | "dfn"
            | "em"
            | "i"
            | "kbd"
            | "label"
            | "mark"
            | "q"
            | "rp"
            | "rt"
            | "ruby"
            | "s"
            | "samp"
            | "small"
            | "span"
            | "strong"
            | "sub"
            | "sup"
            | "time"
            | "u"
            | "var"
            | "wbr"
            | "img"
            | "input"
            | "button"
            | "select"
            | "textarea"
            | "svg"
            | "path"
    )
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
        image: None,
        children,
    }
}

fn load_image_for_layout(
    src: &str,
    loader: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> Option<LayoutImage> {
    let raw = loader(src)?;
    let img = image::load_from_memory(&raw).ok()?.to_rgba8();
    let width = img.width();
    let height = img.height();
    Some(LayoutImage {
        rgba: img.into_raw(),
        width,
        height,
    })
}

fn to_taffy_style(s: &ComputedStyle, vw: f32, vh: f32) -> TaffyStyle {
    let display = match s.display {
        StyleDisplay::None => TaffyDisplay::None,
        StyleDisplay::Flex | StyleDisplay::InlineFlex => TaffyDisplay::Flex,
        StyleDisplay::Grid | StyleDisplay::InlineGrid => {
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
            if matches!(s.display, StyleDisplay::Flex | StyleDisplay::InlineFlex) {
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
        JustifyContent::Unspecified => {
            if matches!(s.display, StyleDisplay::Grid | StyleDisplay::InlineGrid) {
                taffy::style::JustifyContent::Stretch
            } else {
                taffy::style::JustifyContent::FlexStart
            }
        }
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

    let align_self = None;

    TaffyStyle {
        display,
        flex_direction,
        flex_wrap,
        justify_content: Some(justify_content),
        align_items: Some(align_items),
        align_self,
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
        grid_template_columns: parse_grid_tracks(&s.grid_template_columns, vw, vh, None, None, 0.0),
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

fn to_taffy_style_for_node(
    node: &StyledNode,
    vw: f32,
    vh: f32,
    available_content_width: Option<f32>,
) -> TaffyStyle {
    let mut style = to_taffy_style(&node.style, vw, vh);
    let auto_fit_gap = available_content_width
        .map(|width| length_to_px(node.style.gap, width, vw, vh))
        .unwrap_or(0.0);
    style.grid_template_columns = parse_grid_tracks(
        &node.style.grid_template_columns,
        vw,
        vh,
        Some(visible_layout_child_count(node)),
        available_content_width,
        auto_fit_gap,
    );
    if matches!(node.kind, StyledKind::Document) {
        style.size.width = taffy::style::Dimension::Length(vw);
        style.size.height = taffy::style::Dimension::Length(vh);
    }
    style
}

fn visible_layout_child_count(node: &StyledNode) -> usize {
    node.children
        .iter()
        .filter(|child| {
            if child.style.display == StyleDisplay::None {
                return false;
            }
            if let StyledKind::Text(text) = &child.kind {
                return !text.trim().is_empty();
            }
            true
        })
        .count()
}

fn estimate_content_width(
    node: &StyledNode,
    containing_content_width: Option<f32>,
    vw: f32,
    vh: f32,
) -> Option<f32> {
    let containing = containing_content_width?;
    let style = &node.style;

    let horizontal_margin = non_auto_px(style.margin_left, containing, vw, vh)
        + non_auto_px(style.margin_right, containing, vw, vh);
    let horizontal_padding = length_to_px(style.padding_left, containing, vw, vh)
        + length_to_px(style.padding_right, containing, vw, vh);

    let mut border_box_width = match style.width {
        Length::Auto => (containing - horizontal_margin).max(0.0),
        _ => match style.box_sizing {
            BoxSizing::BorderBox => length_to_px(style.width, containing, vw, vh),
            BoxSizing::ContentBox => {
                length_to_px(style.width, containing, vw, vh) + horizontal_padding
            }
        },
    };

    if !style.max_width.is_auto() {
        border_box_width = border_box_width.min(length_to_px(style.max_width, containing, vw, vh));
    }

    Some(match style.box_sizing {
        BoxSizing::BorderBox => (border_box_width - horizontal_padding).max(0.0),
        BoxSizing::ContentBox if style.width.is_auto() => {
            (border_box_width - horizontal_padding).max(0.0)
        }
        BoxSizing::ContentBox => length_to_px(style.width, containing, vw, vh).max(0.0),
    })
}

fn non_auto_px(length: Length, container: f32, vw: f32, vh: f32) -> f32 {
    if length.is_auto() {
        0.0
    } else {
        length_to_px(length, container, vw, vh)
    }
}

fn parse_grid_tracks(
    text: &str,
    vw: f32,
    vh: f32,
    auto_fit_item_count: Option<usize>,
    auto_fit_available_width: Option<f32>,
    auto_fit_gap: f32,
) -> Vec<taffy::style::TrackSizingFunction> {
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
        let track_str = parts[1].trim();
        let tracks = parse_single_track_list(track_str, vw, vh);
        if tracks.is_empty() {
            return Vec::new();
        }
        let count = match parts[0].trim() {
            "auto-fit" => auto_fit_repeat_count(
                &tracks,
                auto_fit_item_count,
                auto_fit_available_width,
                auto_fit_gap,
            )
            .map(GridTrackRepetition::Count)
            .unwrap_or(GridTrackRepetition::AutoFit),
            "auto-fill" => GridTrackRepetition::AutoFill,
            n => match n.parse::<u16>() {
                Ok(num) => GridTrackRepetition::Count(num),
                Err(_) => return Vec::new(),
            },
        };
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

fn auto_fit_repeat_count(
    tracks: &[taffy::style::NonRepeatedTrackSizingFunction],
    item_count: Option<usize>,
    available_width: Option<f32>,
    gap: f32,
) -> Option<u16> {
    let item_count = item_count?.max(1);
    let available_width = available_width?.max(0.0);
    let track_count = tracks.len().max(1) as f32;
    let pattern_width = tracks
        .iter()
        .map(|track| auto_repeat_track_breadth(track, available_width))
        .sum::<f32>()
        .max(track_count);

    let gap = gap.max(0.0);
    let max_fit = ((available_width + gap) / (pattern_width + track_count * gap))
        .floor()
        .max(1.0) as usize;
    u16::try_from(item_count.min(max_fit).max(1)).ok()
}

fn auto_repeat_track_breadth(
    track: &taffy::style::NonRepeatedTrackSizingFunction,
    available_width: f32,
) -> f32 {
    track
        .max
        .definite_value(Some(available_width))
        .or_else(|| track.min.definite_value(Some(available_width)))
        .unwrap_or(1.0)
        .max(1.0)
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

fn length_to_px(l: Length, container: f32, vw: f32, vh: f32) -> f32 {
    match l {
        Length::Px(v) => v,
        Length::Percent(v) => container * v,
        Length::Vw(v) => v * vw / 100.0,
        Length::Vh(v) => v * vh / 100.0,
        Length::Clamp { .. } => l.resolve_px(container, vw),
        Length::Auto | Length::Zero => 0.0,
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
    line_height_px: f32,
    known: Size<Option<f32>>,
    available: Size<AvailableSpace>,
) -> Size<f32> {
    let char_width = font_size * 0.55;

    let avail_width = match available.width {
        AvailableSpace::Definite(w) => w,
        _ => f32::MAX,
    };

    let (natural_width, line_count) = measure_wrapped_text(text, char_width, avail_width);
    let width = known.width.unwrap_or(natural_width.max(char_width));
    let height = known.height.unwrap_or(line_height_px * line_count as f32);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_fit_collapses_empty_tracks_after_counting_available_width() {
        let tracks = parse_single_track_list("minmax(230px, 1fr)", 800.0, 600.0);

        assert_eq!(
            auto_fit_repeat_count(&tracks, Some(2), Some(684.0), 18.0),
            Some(2)
        );
        assert_eq!(
            auto_fit_repeat_count(&tracks, Some(5), Some(684.0), 18.0),
            Some(2)
        );
        assert_eq!(
            auto_fit_repeat_count(&tracks, Some(5), Some(980.0), 18.0),
            Some(4)
        );
    }

    #[test]
    fn auto_fit_keeps_at_least_one_track_when_too_narrow() {
        let tracks = parse_single_track_list("minmax(230px, 1fr)", 320.0, 600.0);

        assert_eq!(
            auto_fit_repeat_count(&tracks, Some(3), Some(180.0), 18.0),
            Some(1)
        );
    }
}
