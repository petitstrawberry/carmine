use taffy::geometry::{Line, Rect, Size};
use taffy::prelude::TaffyGridSpan;
use taffy::style::{
    AvailableSpace, Display as TaffyDisplay, FlexDirection, GridPlacement, LengthPercentage,
    LengthPercentageAuto, MaxTrackSizingFunction, MinTrackSizingFunction,
    NonRepeatedTrackSizingFunction, Style as TaffyStyle, TrackSizingFunction,
};
use taffy::{NodeId, TaffyTree};

use crate::render::style::{
    AlignItems, BoxSizing, ComputedStyle, Display as StyleDisplay, FlexDirection as StyleFlexDir,
    FlexWrap as StyleFlexWrap, JustifyContent, Length, StyledKind, StyledNode, WhiteSpace,
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
    pub inline_fragments: Vec<LayoutTextFragment>,
    pub image: Option<LayoutImage>,
    pub background_image: Option<LayoutImage>,
    pub children: Vec<LayoutNode>,
}

#[derive(Clone, Debug)]
pub struct LayoutTextFragment {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub text: String,
    pub style: ComputedStyle,
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

#[derive(Clone)]
struct TextMeasureData {
    text: String,
    inline_runs: Vec<InlineTextRun>,
    font_size: f32,
    line_height_px: f32,
    white_space: WhiteSpace,
    allow_wrap: bool,
}

#[derive(Clone)]
enum MeasureData {
    Text(TextMeasureData),
    Image {
        intrinsic_width: f32,
        intrinsic_height: f32,
    },
}

#[derive(Clone, Debug)]
struct InlineTextRun {
    text: String,
    style: ComputedStyle,
}

struct TaffyLink<'a> {
    styled: &'a StyledNode,
    taffy_id: NodeId,
    children: Vec<TaffyLink<'a>>,
    text: Option<String>,
    inline_runs: Vec<InlineTextRun>,
    image: Option<LayoutImage>,
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
    let mut tree: TaffyTree<MeasureData> = TaffyTree::new();
    let vw = viewport_w as f32;
    let vh = viewport_h as f32;
    let image_loader_dyn =
        image_loader.map(|loader| loader.as_ref() as &dyn Fn(&str) -> Option<Vec<u8>>);
    let link = build_taffy_tree(root, &mut tree, vw, vh, Some(vw), image_loader_dyn);

    let available = Size {
        width: AvailableSpace::Definite(vw),
        height: AvailableSpace::MaxContent,
    };

    tree.compute_layout_with_measure(
        link.taffy_id,
        available,
        |known, avail, _id, context, _style| match context {
            Some(MeasureData::Text(data)) => measure_text(
                &data.text,
                &data.inline_runs,
                data.font_size,
                data.line_height_px,
                data.white_space,
                data.allow_wrap,
                known,
                avail,
            ),
            Some(MeasureData::Image {
                intrinsic_width,
                intrinsic_height,
            }) => measure_image(*intrinsic_width, *intrinsic_height, known, avail),
            None => Size::ZERO,
        },
    )
    .expect("taffy layout failed");

    let mut root_layout = extract(&link, &tree, 0.0, 0.0);
    if let Some(loader) = image_loader_dyn {
        inject_images(&mut root_layout, &link, loader);
    }
    root_layout
}

fn inject_images(
    node: &mut LayoutNode,
    link: &TaffyLink,
    loader: &dyn Fn(&str) -> Option<Vec<u8>>,
) {
    if let Some(ref src) = link.styled.style.background_image_src {
        node.background_image = load_image_for_layout(src, loader);
    }
    for (child_layout, child_link) in node.children.iter_mut().zip(link.children.iter()) {
        inject_images(child_layout, child_link, loader);
    }
}

fn build_taffy_tree<'a>(
    node: &'a StyledNode,
    tree: &mut TaffyTree<MeasureData>,
    vw: f32,
    vh: f32,
    containing_content_width: Option<f32>,
    image_loader: Option<&dyn Fn(&str) -> Option<Vec<u8>>>,
) -> TaffyLink<'a> {
    build_taffy_tree_inner(
        node,
        tree,
        vw,
        vh,
        containing_content_width,
        true,
        image_loader,
    )
}

fn build_taffy_tree_inner<'a>(
    node: &'a StyledNode,
    tree: &mut TaffyTree<MeasureData>,
    vw: f32,
    vh: f32,
    containing_content_width: Option<f32>,
    parent_stretches: bool,
    image_loader: Option<&dyn Fn(&str) -> Option<Vec<u8>>>,
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
            inline_runs: Vec::new(),
            image: None,
        };
    }

    if let StyledKind::Element { tag } = &node.kind {
        if tag.eq_ignore_ascii_case("table") && node.style.display == StyleDisplay::Flex {
            return build_table_taffy_tree(
                node,
                tree,
                vw,
                vh,
                containing_content_width,
                image_loader,
            );
        }
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
    let should_collapse_inline_text = !has_block_children
        && !visible_children.is_empty()
        && visible_children
            .iter()
            .all(|child| can_inline_format(child));

    let self_stretches = matches!(
        node.style.align_items,
        crate::render::style::AlignItems::Stretch | crate::render::style::AlignItems::Unspecified
    );

    if should_collapse_inline_text {
        let inline_runs = normalize_inline_runs(flatten_inline_runs(node));
        let text = inline_runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();
        if !text.trim().is_empty() {
            let allow_wrap = !matches!(node.style.display, StyleDisplay::Inline);
            let ctx = TextMeasureData {
                text: text.clone(),
                inline_runs: inline_runs.clone(),
                font_size: node.style.font_size,
                line_height_px: node.style.line_height.resolve_px(node.style.font_size),
                white_space: node.style.white_space,
                allow_wrap,
            };
            let mut style = to_taffy_style_for_node(node, vw, vh, node_content_width);
            if parent_stretches && matches!(node.style.display, StyleDisplay::Inline) {
                style.align_self = Some(taffy::style::AlignItems::FlexStart);
            }
            if !allow_wrap {
                style.flex_shrink = 0.0;
            }
            let id = tree
                .new_leaf_with_context(style, MeasureData::Text(ctx))
                .expect("taffy new_leaf_with_context");
            return TaffyLink {
                styled: node,
                taffy_id: id,
                children: vec![],
                text: Some(text),
                inline_runs,
                image: None,
            };
        }
    }

    if visible_children.is_empty() {
        let mut leaf_style = to_taffy_style_for_node(node, vw, vh, node_content_width);
        if parent_stretches && matches!(node.style.display, StyleDisplay::Inline) {
            leaf_style.align_self = Some(taffy::style::AlignItems::FlexStart);
        }
        let (id, image) = match &node.kind {
            StyledKind::Text(text) => {
                let text = normalize_inline_text_node(text, node.style.white_space);
                leaf_style.flex_shrink = 0.0;
                let ctx = TextMeasureData {
                    text: text.clone(),
                    inline_runs: vec![InlineTextRun {
                        text,
                        style: node.style.clone(),
                    }],
                    font_size: node.style.font_size,
                    line_height_px: node.style.line_height.resolve_px(node.style.font_size),
                    white_space: node.style.white_space,
                    allow_wrap: false,
                };
                (
                    tree.new_leaf_with_context(leaf_style, MeasureData::Text(ctx))
                        .expect("taffy new_leaf_with_context"),
                    None,
                )
            }
            _ if node.img_src.is_some() => {
                let image = node.img_src.as_ref().and_then(|src| {
                    image_loader.and_then(|loader| load_image_for_layout(src, loader))
                });
                if let Some(image) = image {
                    let id = tree
                        .new_leaf_with_context(
                            leaf_style,
                            MeasureData::Image {
                                intrinsic_width: image.width as f32,
                                intrinsic_height: image.height as f32,
                            },
                        )
                        .expect("taffy image leaf");
                    (id, Some(image))
                } else {
                    (tree.new_leaf(leaf_style).expect("taffy new_leaf"), None)
                }
            }
            _ => (tree.new_leaf(leaf_style).expect("taffy new_leaf"), None),
        };
        TaffyLink {
            styled: node,
            taffy_id: id,
            children: vec![],
            text: None,
            inline_runs: Vec::new(),
            image,
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
                image_loader,
            )
        } else {
            visible_children
                .iter()
                .map(|c| {
                    build_taffy_tree_inner(
                        c,
                        tree,
                        vw,
                        vh,
                        node_content_width,
                        self_stretches,
                        image_loader,
                    )
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
            container_style.align_items = Some(taffy::style::AlignItems::FlexStart);
            container_style.align_content = Some(taffy::style::AlignContent::FlexStart);
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
            inline_runs: Vec::new(),
            image: None,
        }
    }
}

fn group_children_for_block_layout<'a>(
    children: &[&'a StyledNode],
    tree: &mut TaffyTree<MeasureData>,
    vw: f32,
    vh: f32,
    containing_width: Option<f32>,
    parent_stretches: bool,
    image_loader: Option<&dyn Fn(&str) -> Option<Vec<u8>>>,
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
                        build_taffy_tree_inner(
                            c,
                            tree,
                            vw,
                            vh,
                            containing_width,
                            parent_stretches,
                            image_loader,
                        )
                    })
                    .collect::<Vec<_>>();
                let run_ids: Vec<NodeId> = run_links.iter().map(|c| c.taffy_id).collect();
                let row_id = tree
                    .new_with_children(
                        TaffyStyle {
                            display: TaffyDisplay::Flex,
                            flex_direction: FlexDirection::Row,
                            flex_wrap: taffy::style::FlexWrap::Wrap,
                            align_items: Some(taffy::style::AlignItems::FlexStart),
                            align_content: Some(taffy::style::AlignContent::FlexStart),
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
                    inline_runs: Vec::new(),
                    image: None,
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
                image_loader,
            ));
        }
    }

    if !inline_run.is_empty() {
        let run_links = inline_run
            .iter()
            .map(|c| {
                build_taffy_tree_inner(
                    c,
                    tree,
                    vw,
                    vh,
                    containing_width,
                    parent_stretches,
                    image_loader,
                )
            })
            .collect::<Vec<_>>();
        let run_ids: Vec<NodeId> = run_links.iter().map(|c| c.taffy_id).collect();
        let row_id = tree
            .new_with_children(
                TaffyStyle {
                    display: TaffyDisplay::Flex,
                    flex_direction: FlexDirection::Row,
                    flex_wrap: taffy::style::FlexWrap::Wrap,
                    align_items: Some(taffy::style::AlignItems::FlexStart),
                    align_content: Some(taffy::style::AlignContent::FlexStart),
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
            inline_runs: Vec::new(),
            image: None,
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

fn can_inline_format(node: &StyledNode) -> bool {
    match &node.kind {
        StyledKind::Text(_) => true,
        StyledKind::Element { tag } if tag.eq_ignore_ascii_case("br") => true,
        StyledKind::Element { tag } => {
            is_inline_text_container_tag(tag)
                && node
                    .children
                    .iter()
                    .filter(|child| child.style.display != StyleDisplay::None)
                    .all(can_inline_format)
        }
        StyledKind::Document => false,
    }
}

fn is_inline_text_container_tag(tag: &str) -> bool {
    matches!(
        tag.to_lowercase().as_str(),
        "a" | "abbr"
            | "b"
            | "bdi"
            | "bdo"
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
    )
}

fn flatten_inline_runs(node: &StyledNode) -> Vec<InlineTextRun> {
    match &node.kind {
        StyledKind::Text(text) => vec![InlineTextRun {
            text: text.clone(),
            style: node.style.clone(),
        }],
        StyledKind::Element { tag } if tag.eq_ignore_ascii_case("br") => vec![InlineTextRun {
            text: "\n".to_string(),
            style: node.style.clone(),
        }],
        StyledKind::Element { .. } | StyledKind::Document => node
            .children
            .iter()
            .filter(|child| child.style.display != StyleDisplay::None)
            .flat_map(flatten_inline_runs)
            .collect(),
    }
}

fn normalize_inline_runs(runs: Vec<InlineTextRun>) -> Vec<InlineTextRun> {
    let mut normalized = Vec::new();
    let mut previous_was_space = true;

    for run in runs {
        let mut text = String::new();
        for ch in run.text.chars() {
            if ch == '\n' {
                if text.ends_with(' ') {
                    text.pop();
                }
                text.push('\n');
                previous_was_space = true;
            } else if ch.is_whitespace() {
                if !previous_was_space {
                    text.push(' ');
                }
                previous_was_space = true;
            } else {
                text.push(ch);
                previous_was_space = false;
            }
        }

        if !text.is_empty() {
            push_inline_run(
                &mut normalized,
                InlineTextRun {
                    text,
                    style: run.style,
                },
            );
        }
    }

    if let Some(last) = normalized.last_mut() {
        if last.text.ends_with(' ') {
            last.text.pop();
        }
    }
    normalized.retain(|run| !run.text.is_empty());
    normalized
}

fn push_inline_run(runs: &mut Vec<InlineTextRun>, run: InlineTextRun) {
    if let Some(last) = runs.last_mut() {
        if text_style_matches(&last.style, &run.style) {
            last.text.push_str(&run.text);
            return;
        }
    }
    runs.push(run);
}

fn text_style_matches(a: &ComputedStyle, b: &ComputedStyle) -> bool {
    a.font_size == b.font_size
        && a.font_weight == b.font_weight
        && a.color.r == b.color.r
        && a.color.g == b.color.g
        && a.color.b == b.color.b
        && a.color.a == b.color.a
        && a.letter_spacing == b.letter_spacing
        && a.text_transform == b.text_transform
        && a.text_decoration == b.text_decoration
        && a.line_height == b.line_height
        && a.white_space == b.white_space
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

fn collapse_inline_text_node_whitespace(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut previous_was_space = false;

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

    result
}

fn normalize_html_text(text: &str, white_space: WhiteSpace) -> String {
    match white_space {
        WhiteSpace::Normal | WhiteSpace::Nowrap => collapse_html_whitespace(text),
        WhiteSpace::Pre | WhiteSpace::PreWrap => text.to_string(),
    }
}

fn normalize_inline_text_node(text: &str, white_space: WhiteSpace) -> String {
    match white_space {
        WhiteSpace::Normal | WhiteSpace::Nowrap => collapse_inline_text_node_whitespace(text),
        WhiteSpace::Pre | WhiteSpace::PreWrap => text.to_string(),
    }
}

fn extract(
    link: &TaffyLink,
    tree: &TaffyTree<MeasureData>,
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
            StyledKind::Text(t) => Some(normalize_html_text(t, link.styled.style.white_space)),
            _ => None,
        }
    };

    let tag = match &link.styled.kind {
        StyledKind::Element { tag } => Some(tag.clone()),
        _ => None,
    };

    let inline_fragments = if !link.children.is_empty() {
        Vec::new()
    } else if !link.inline_runs.is_empty() {
        layout_inline_fragments(&link.inline_runs, layout.size.width, &link.styled.style)
    } else {
        Vec::new()
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
        inline_fragments,
        image: link.image.clone(),
        background_image: None,
        children,
    }
}

fn measure_image(
    intrinsic_width: f32,
    intrinsic_height: f32,
    known: Size<Option<f32>>,
    available: Size<AvailableSpace>,
) -> Size<f32> {
    if intrinsic_width <= 0.0 || intrinsic_height <= 0.0 {
        return Size::ZERO;
    }

    let aspect = intrinsic_height / intrinsic_width;
    let avail_width = match available.width {
        AvailableSpace::Definite(w) => Some(w),
        _ => None,
    };

    let (mut width, mut height) = match (known.width, known.height) {
        (Some(w), Some(h)) => (w, h),
        (Some(w), None) => (w, w * aspect),
        (None, Some(h)) => (h / aspect, h),
        (None, None) => (intrinsic_width, intrinsic_height),
    };

    if known.width.is_none() {
        if let Some(max_width) = avail_width {
            if width > max_width && max_width > 0.0 {
                width = max_width;
                height = width * aspect;
            }
        }
    }

    Size { width, height }
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
            } else if matches!(s.display, StyleDisplay::Inline) {
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
        flex_grow: s.flex_grow,
        flex_shrink: s.flex_shrink,
        flex_basis: length_to_dim(s.flex_basis, vw, vh),
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
    inline_runs: &[InlineTextRun],
    font_size: f32,
    line_height_px: f32,
    white_space: WhiteSpace,
    allow_wrap: bool,
    known: Size<Option<f32>>,
    available: Size<AvailableSpace>,
) -> Size<f32> {
    let char_width = font_size * 0.55;

    let avail_width = match available.width {
        AvailableSpace::Definite(w) => w,
        _ => f32::MAX,
    };

    let (natural_width, line_count) = if !inline_runs.is_empty() {
        measure_inline_runs(inline_runs, avail_width, allow_wrap, white_space)
    } else if !allow_wrap {
        (text.chars().count() as f32 * char_width, 1)
    } else {
        match white_space {
            WhiteSpace::Nowrap => (text.chars().count() as f32 * char_width, 1),
            WhiteSpace::Pre => measure_pre_text(text, char_width),
            WhiteSpace::PreWrap => measure_pre_wrap_text(text, char_width, avail_width),
            WhiteSpace::Normal => measure_wrapped_text(text, char_width, avail_width),
        }
    };
    let width = known.width.unwrap_or(natural_width.max(char_width));
    let height = known.height.unwrap_or(line_height_px * line_count as f32);

    Size { width, height }
}

fn measure_pre_text(text: &str, char_width: f32) -> (f32, usize) {
    let mut max_width = 0.0f32;
    let mut lines = 0usize;
    for line in text.split('\n') {
        lines += 1;
        max_width = max_width.max(line.chars().count() as f32 * char_width);
    }
    (max_width, lines.max(1))
}

fn measure_pre_wrap_text(text: &str, char_width: f32, max_width: f32) -> (f32, usize) {
    let mut natural_width = 0.0f32;
    let mut line_count = 0usize;
    for line in text.split('\n') {
        let (width, lines) = measure_wrapped_text(line, char_width, max_width);
        natural_width = natural_width.max(width);
        line_count += lines;
    }
    (natural_width, line_count.max(1))
}

fn measure_inline_runs(
    runs: &[InlineTextRun],
    max_width: f32,
    allow_wrap: bool,
    white_space: WhiteSpace,
) -> (f32, usize) {
    let fragments = layout_inline_fragments_with_options(runs, max_width, allow_wrap, white_space);
    if fragments.is_empty() {
        return (0.0, 1);
    }

    let mut max_line_width = 0.0f32;
    let mut line_count = 1usize;
    let mut last_y = fragments[0].y;
    for fragment in &fragments {
        if fragment.y > last_y {
            line_count += 1;
            last_y = fragment.y;
        }
        max_line_width = max_line_width.max(fragment.x + fragment.width);
    }
    (max_line_width, line_count)
}

fn layout_inline_fragments(
    runs: &[InlineTextRun],
    max_width: f32,
    parent_style: &ComputedStyle,
) -> Vec<LayoutTextFragment> {
    layout_inline_fragments_with_options(
        runs,
        max_width,
        !matches!(
            parent_style.white_space,
            WhiteSpace::Nowrap | WhiteSpace::Pre
        ),
        parent_style.white_space,
    )
}

fn layout_inline_fragments_with_options(
    runs: &[InlineTextRun],
    max_width: f32,
    allow_wrap: bool,
    white_space: WhiteSpace,
) -> Vec<LayoutTextFragment> {
    let nowrap = !allow_wrap || matches!(white_space, WhiteSpace::Nowrap | WhiteSpace::Pre);
    let mut fragments = Vec::new();
    let mut x = 0.0f32;
    let mut y = 0.0f32;
    let mut line_height = runs
        .first()
        .map(|run| run.style.line_height.resolve_px(run.style.font_size))
        .unwrap_or(16.0 * 1.2);

    for token in inline_tokens(runs) {
        if token.text == "\n" && matches!(white_space, WhiteSpace::Pre | WhiteSpace::PreWrap) {
            x = 0.0;
            y += line_height;
            continue;
        }

        let token_width = token_width(&token);
        if !nowrap && x > 0.0 && x + token_width > max_width && max_width.is_finite() {
            x = 0.0;
            y += line_height;
        }

        if token.text == " " && x == 0.0 {
            continue;
        }

        line_height = line_height.max(token.style.line_height.resolve_px(token.style.font_size));
        push_fragment(&mut fragments, x, y, token_width, line_height, token);
        x += token_width;
    }

    fragments
}

fn inline_tokens(runs: &[InlineTextRun]) -> Vec<InlineTextRun> {
    let mut tokens = Vec::new();
    let mut current: Option<InlineTextRun> = None;

    for run in runs {
        for ch in run.text.chars() {
            if ch == ' ' || ch == '\n' {
                if let Some(token) = current.take() {
                    tokens.push(token);
                }
                tokens.push(InlineTextRun {
                    text: ch.to_string(),
                    style: run.style.clone(),
                });
                continue;
            }

            if is_cjk_char(ch) {
                if let Some(token) = current.take() {
                    tokens.push(token);
                }
                tokens.push(InlineTextRun {
                    text: ch.to_string(),
                    style: run.style.clone(),
                });
                continue;
            }

            match current.as_mut() {
                Some(token) if text_style_matches(&token.style, &run.style) => token.text.push(ch),
                Some(_) => {
                    tokens.push(current.take().unwrap());
                    current = Some(InlineTextRun {
                        text: ch.to_string(),
                        style: run.style.clone(),
                    });
                }
                None => {
                    current = Some(InlineTextRun {
                        text: ch.to_string(),
                        style: run.style.clone(),
                    });
                }
            }
        }
    }

    if let Some(token) = current {
        tokens.push(token);
    }
    tokens
}

fn is_cjk_char(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3040..=0x309f
            | 0x30a0..=0x30ff
            | 0x3400..=0x4dbf
            | 0x4e00..=0x9fff
            | 0xf900..=0xfaff
            | 0xff00..=0xffef
    )
}

fn token_width(token: &InlineTextRun) -> f32 {
    token
        .text
        .chars()
        .map(|ch| {
            let advance = if ch == ' ' { 0.33 } else { 0.55 } * token.style.font_size;
            advance + token.style.letter_spacing
        })
        .sum()
}

fn push_fragment(
    fragments: &mut Vec<LayoutTextFragment>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    token: InlineTextRun,
) {
    if let Some(last) = fragments.last_mut() {
        if last.y == y && text_style_matches(&last.style, &token.style) {
            last.text.push_str(&token.text);
            last.width += width;
            return;
        }
    }
    fragments.push(LayoutTextFragment {
        x,
        y,
        width,
        height,
        text: token.text,
        style: token.style,
    });
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

fn build_table_taffy_tree<'a>(
    node: &'a StyledNode,
    tree: &mut TaffyTree<MeasureData>,
    vw: f32,
    vh: f32,
    containing_content_width: Option<f32>,
    image_loader: Option<&dyn Fn(&str) -> Option<Vec<u8>>>,
) -> TaffyLink<'a> {
    let node_content_width = estimate_content_width(node, containing_content_width, vw, vh);

    let mut table_style = to_taffy_style_for_node(node, vw, vh, node_content_width);
    table_style.display = TaffyDisplay::Grid;

    let cells = collect_table_cells_with_columns(node);
    let max_columns = count_max_columns(node);
    let mut estimated_col_widths = Vec::new();
    if max_columns > 0 {
        let col_widths = collect_column_width_hints(node, max_columns);
        table_style.grid_template_columns = build_grid_tracks_from_hints(&col_widths, max_columns);
        if let Some(width) = node_content_width {
            let intrinsic_col_widths = collect_intrinsic_column_widths(node, max_columns, vw, vh);
            estimated_col_widths = estimate_column_widths_from_hints(
                &col_widths,
                max_columns,
                width,
                &intrinsic_col_widths,
            );
            if let Some(table_width) =
                estimate_auto_table_width(&estimated_col_widths, width, node.style.width)
            {
                table_style.size.width = taffy::style::Dimension::Length(table_width);
            }
        }
    }

    let mut child_links: Vec<TaffyLink<'a>> = Vec::with_capacity(cells.len());
    for (cell_node, start_col) in cells {
        let cell_colspan = cell_node.style.table_colspan.max(1) as usize;
        let cell_estimated_width = estimate_cell_width(
            &estimated_col_widths,
            start_col,
            cell_colspan,
            node_content_width,
            max_columns,
        );
        let cell_link = build_taffy_tree_inner(
            cell_node,
            tree,
            vw,
            vh,
            cell_estimated_width,
            true,
            image_loader,
        );

        let colspan = cell_node.style.table_colspan.max(1) as u16;
        let rowspan = cell_node.style.table_rowspan.max(1) as u16;
        if colspan > 1 || rowspan > 1 {
            if let Ok(mut style) = tree.style(cell_link.taffy_id).cloned() {
                if colspan > 1 {
                    style.grid_column = Line {
                        start: GridPlacement::Auto,
                        end: GridPlacement::from_span(colspan),
                    };
                }
                if rowspan > 1 {
                    style.grid_row = Line {
                        start: GridPlacement::Auto,
                        end: GridPlacement::from_span(rowspan),
                    };
                }
                tree.set_style(cell_link.taffy_id, style)
                    .expect("taffy set_style for table cell span");
            }
        }

        child_links.push(cell_link);
    }

    let child_ids: Vec<NodeId> = child_links.iter().map(|c| c.taffy_id).collect();
    let id = tree
        .new_with_children(table_style, &child_ids)
        .expect("taffy table grid node");

    TaffyLink {
        styled: node,
        taffy_id: id,
        children: child_links,
        text: None,
        inline_runs: Vec::new(),
        image: None,
    }
}

fn collect_table_cells_with_columns(table: &StyledNode) -> Vec<(&StyledNode, usize)> {
    let rows = collect_table_rows(table);
    let mut cells = Vec::new();

    for row in rows {
        let mut col = 0usize;
        for cell in row.children.iter().filter(|c| is_table_cell(c)) {
            cells.push((cell, col));
            col += cell.style.table_colspan.max(1) as usize;
        }
    }

    cells
}

fn estimate_cell_width(
    col_widths: &[f32],
    start_col: usize,
    colspan: usize,
    table_width: Option<f32>,
    max_columns: usize,
) -> Option<f32> {
    if !col_widths.is_empty() {
        let width = col_widths
            .iter()
            .skip(start_col)
            .take(colspan.max(1))
            .sum::<f32>();
        return Some(width.max(1.0));
    }

    table_width.map(|w| {
        let cols = max_columns.max(1) as f32;
        (w * colspan.max(1) as f32 / cols).max(1.0)
    })
}

fn estimate_auto_table_width(
    col_widths: &[f32],
    available_width: f32,
    style_width: Length,
) -> Option<f32> {
    if !style_width.is_auto() {
        return None;
    }

    let intrinsic_width = col_widths.iter().sum::<f32>();
    (intrinsic_width > available_width).then_some(intrinsic_width)
}

fn is_table_cell(node: &StyledNode) -> bool {
    if node.style.display == StyleDisplay::None {
        return false;
    }
    matches!(
        &node.kind,
        StyledKind::Element { tag }
            if tag.eq_ignore_ascii_case("td") || tag.eq_ignore_ascii_case("th")
    )
}

fn count_max_columns(table: &StyledNode) -> usize {
    let rows = collect_table_rows(table);
    rows.into_iter()
        .map(|row| {
            row.children
                .iter()
                .filter(|c| is_table_cell(c))
                .map(|c| c.style.table_colspan.max(1) as usize)
                .sum::<usize>()
        })
        .max()
        .unwrap_or(0)
}

fn collect_column_width_hints(table: &StyledNode, max_columns: usize) -> Vec<Option<Length>> {
    let rows = collect_table_rows(table);
    let mut hints: Vec<Option<Length>> = vec![None; max_columns];

    for row in &rows {
        let mut col = 0usize;
        for cell in row.children.iter().filter(|c| is_table_cell(c)) {
            let colspan = cell.style.table_colspan.max(1) as usize;
            if !cell.style.width.is_auto() && cell.style.width != Length::Zero {
                let w = cell.style.width;
                let per_col = match (w, colspan) {
                    (Length::Percent(v), n) if n > 1 => Length::Percent(v / n as f32),
                    (Length::Px(v), n) if n > 1 => Length::Px(v / n as f32),
                    (other, _) => other,
                };
                for _ in 0..colspan {
                    if col < max_columns && hints[col].is_none() {
                        hints[col] = Some(per_col);
                    }
                    col += 1;
                }
            } else {
                col += colspan;
            }
        }
    }
    hints
}

fn build_grid_tracks_from_hints(
    hints: &[Option<Length>],
    count: usize,
) -> Vec<TrackSizingFunction> {
    (0..count.max(1))
        .map(|i| match hints.get(i).and_then(|h| h.as_ref()) {
            Some(Length::Percent(v)) => {
                TrackSizingFunction::Single(NonRepeatedTrackSizingFunction {
                    min: MinTrackSizingFunction::Fixed(LengthPercentage::Percent(*v)),
                    max: MaxTrackSizingFunction::Auto,
                })
            }
            Some(Length::Px(v)) => TrackSizingFunction::Single(NonRepeatedTrackSizingFunction {
                min: MinTrackSizingFunction::Fixed(LengthPercentage::Length(*v)),
                max: MaxTrackSizingFunction::Auto,
            }),
            _ => TrackSizingFunction::Single(NonRepeatedTrackSizingFunction {
                min: MinTrackSizingFunction::Auto,
                max: MaxTrackSizingFunction::Fraction(1.0),
            }),
        })
        .collect()
}

fn estimate_column_widths_from_hints(
    hints: &[Option<Length>],
    count: usize,
    table_width: f32,
    intrinsic_widths: &[f32],
) -> Vec<f32> {
    let count = count.max(1);
    let mut widths = vec![0.0; count];
    let mut fixed_width = 0.0f32;
    let mut flexible_columns = 0usize;

    for (index, width) in widths.iter_mut().enumerate() {
        match hints.get(index).and_then(|h| h.as_ref()) {
            Some(Length::Percent(v)) => {
                *width = table_width * *v;
                fixed_width += *width;
            }
            Some(Length::Px(v)) => {
                *width = *v;
                fixed_width += *width;
            }
            _ => {
                let intrinsic = intrinsic_widths.get(index).copied().unwrap_or(0.0);
                *width = intrinsic;
                fixed_width += intrinsic;
                flexible_columns += 1;
            }
        }
    }

    if flexible_columns > 0 {
        let remaining_width = (table_width - fixed_width).max(0.0);
        let flexible_width = if remaining_width > 0.0 {
            (remaining_width / flexible_columns as f32).max(1.0)
        } else {
            0.0
        };
        for (index, width) in widths.iter_mut().enumerate() {
            if hints.get(index).and_then(|h| h.as_ref()).is_none() {
                *width += flexible_width;
            }
        }
    }

    widths
}

fn collect_intrinsic_column_widths(
    table: &StyledNode,
    max_columns: usize,
    vw: f32,
    vh: f32,
) -> Vec<f32> {
    let rows = collect_table_rows(table);
    let mut widths = vec![0.0f32; max_columns];

    for row in rows {
        let mut col = 0usize;
        for cell in row.children.iter().filter(|c| is_table_cell(c)) {
            let colspan = cell.style.table_colspan.max(1) as usize;
            let per_col = intrinsic_node_width(cell, vw, vh) / colspan as f32;
            for offset in 0..colspan {
                if let Some(width) = widths.get_mut(col + offset) {
                    *width = (*width).max(per_col);
                }
            }
            col += colspan;
        }
    }

    widths
}

fn intrinsic_node_width(node: &StyledNode, vw: f32, vh: f32) -> f32 {
    if node.style.display == StyleDisplay::None {
        return 0.0;
    }

    let specified = definite_intrinsic_width(node.style.width, vw, vh);
    let horizontal_padding = length_to_px(node.style.padding_left, vw, vw, vh)
        + length_to_px(node.style.padding_right, vw, vw, vh);
    let horizontal_border = node.style.border_width * 2.0
        + node.style.border_left_width
        + node.style.border_right_width;
    let children = intrinsic_children_width(node, vw, vh);

    specified.max(children + horizontal_padding + horizontal_border)
}

fn intrinsic_children_width(node: &StyledNode, vw: f32, vh: f32) -> f32 {
    match &node.kind {
        StyledKind::Text(text) => intrinsic_text_width(text, &node.style),
        StyledKind::Element { .. } | StyledKind::Document => {
            if can_inline_format(node) {
                let runs = normalize_inline_runs(flatten_inline_runs(node));
                measure_inline_runs(&runs, f32::MAX, false, node.style.white_space).0
            } else {
                node.children
                    .iter()
                    .map(|child| intrinsic_node_width(child, vw, vh))
                    .fold(0.0, f32::max)
            }
        }
    }
}

fn intrinsic_text_width(text: &str, style: &ComputedStyle) -> f32 {
    let text = normalize_html_text(text, style.white_space);
    let run = InlineTextRun {
        text,
        style: style.clone(),
    };
    measure_inline_runs(&[run], f32::MAX, false, style.white_space).0
}

fn definite_intrinsic_width(width: Length, vw: f32, vh: f32) -> f32 {
    match width {
        Length::Px(v) => v,
        Length::Vw(v) => v * vw / 100.0,
        Length::Vh(v) => v * vh / 100.0,
        Length::Clamp { .. } => width.resolve_px(vw, vw),
        Length::Zero | Length::Auto | Length::Percent(_) => 0.0,
    }
}

fn collect_table_rows(table: &StyledNode) -> Vec<&StyledNode> {
    let mut rows = Vec::new();
    for child in &table.children {
        if let StyledKind::Element { tag } = &child.kind {
            let t = tag.to_ascii_lowercase();
            if t == "tr" {
                rows.push(child);
            } else if t == "thead" || t == "tbody" || t == "tfoot" {
                for gc in &child.children {
                    if let StyledKind::Element { tag } = &gc.kind {
                        if tag.eq_ignore_ascii_case("tr") {
                            rows.push(gc);
                        }
                    }
                }
            }
        }
    }
    rows
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

    #[test]
    fn table_cell_width_estimate_uses_remaining_auto_column_width() {
        let hints = vec![
            Some(Length::Percent(0.25)),
            None,
            Some(Length::Percent(0.25)),
        ];
        let columns = estimate_column_widths_from_hints(&hints, 3, 800.0, &[]);

        assert_eq!(columns, vec![200.0, 400.0, 200.0]);
        assert_eq!(
            estimate_cell_width(&columns, 1, 1, Some(800.0), 3),
            Some(400.0)
        );
        assert!(estimate_cell_width(&columns, 1, 1, Some(800.0), 3).unwrap() > 800.0 / 3.0);
    }

    #[test]
    fn table_auto_column_estimate_honors_intrinsic_content_width() {
        let hints = vec![
            Some(Length::Percent(0.25)),
            None,
            Some(Length::Percent(0.25)),
        ];
        let intrinsic = vec![0.0, 520.0, 0.0];
        let columns = estimate_column_widths_from_hints(&hints, 3, 800.0, &intrinsic);

        assert_eq!(columns, vec![200.0, 520.0, 200.0]);
        assert_eq!(
            estimate_cell_width(&columns, 1, 1, Some(800.0), 3),
            Some(520.0)
        );
    }

    #[test]
    fn auto_table_width_expands_to_intrinsic_columns() {
        assert_eq!(
            estimate_auto_table_width(&[200.0, 520.0, 200.0], 800.0, Length::Auto),
            Some(920.0)
        );
        assert_eq!(
            estimate_auto_table_width(&[200.0, 400.0, 200.0], 800.0, Length::Auto),
            None
        );
        assert_eq!(
            estimate_auto_table_width(&[200.0, 520.0, 200.0], 800.0, Length::Px(800.0)),
            None
        );
    }

    #[test]
    fn nowrap_measurement_keeps_text_on_one_line() {
        let size = measure_text(
            "Google 検索",
            &[],
            16.0,
            20.0,
            WhiteSpace::Nowrap,
            true,
            Size::NONE,
            Size {
                width: AvailableSpace::Definite(30.0),
                height: AvailableSpace::MaxContent,
            },
        );

        assert_eq!(size.height, 20.0);
        assert!(size.width > 30.0);
    }

    #[test]
    fn pre_measurement_preserves_hard_lines() {
        let (width, lines) = measure_pre_text("abc\ndef", 10.0);

        assert_eq!(width, 30.0);
        assert_eq!(lines, 2);
    }

    #[test]
    fn inline_fragments_preserve_strong_style_and_punctuation_flow() {
        let normal = ComputedStyle {
            font_size: 16.0,
            ..crate::render::style::default_block_for_test()
        };
        let strong = ComputedStyle {
            font_size: 16.0,
            font_weight: 700,
            ..crate::render::style::default_block_for_test()
        };
        let runs = vec![
            InlineTextRun {
                text: "Confirmed path: ".to_string(),
                style: normal.clone(),
            },
            InlineTextRun {
                text: "/var/www/html/index.html".to_string(),
                style: strong,
            },
            InlineTextRun {
                text: ". You are not hitting a host web server.".to_string(),
                style: normal,
            },
        ];

        let fragments = layout_inline_fragments_with_options(
            &normalize_inline_runs(runs),
            800.0,
            true,
            WhiteSpace::Normal,
        );

        let rendered = fragments
            .iter()
            .map(|f| f.text.as_str())
            .collect::<String>();
        assert_eq!(
            rendered,
            "Confirmed path: /var/www/html/index.html. You are not hitting a host web server."
        );
        assert!(
            fragments
                .iter()
                .any(|fragment| fragment.text == "/var/www/html/index.html"
                    && fragment.style.font_weight == 700)
        );
        assert!(!rendered.contains("html ."));
    }

    #[test]
    fn inline_fragments_allow_cjk_line_breaks_without_spaces() {
        let style = ComputedStyle {
            font_size: 16.0,
            ..crate::render::style::default_block_for_test()
        };
        let runs = vec![InlineTextRun {
            text: "あなたがこのページを開いたということですか".to_string(),
            style,
        }];

        let fragments = layout_inline_fragments_with_options(
            &normalize_inline_runs(runs),
            80.0,
            true,
            WhiteSpace::Normal,
        );

        assert!(fragments.iter().any(|fragment| fragment.y > 0.0));
    }
}
