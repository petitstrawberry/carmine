use std::sync::OnceLock;

use ab_glyph::{Font, FontVec, Glyph, OutlinedGlyph, ScaleFont};
use tiny_skia::{Color as SkColor, Paint, PathBuilder, Pixmap, Rect};

use crate::render::layout::LayoutNode;
use crate::render::style::{
    Background, BackgroundPositionAxis, BoxShadow, Color, ComputedStyle, TextAlign, TextDecoration,
    Visibility, WhiteSpace,
};

static FONTS: OnceLock<FontBook> = OnceLock::new();

struct FontBook {
    fonts: Vec<FontVec>,
}

fn load_fonts() -> FontBook {
    let paths = [
        "/fonts/Mplus1-Regular.ttf",
        "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
        "/System/Library/Fonts/ヒラギノ角ゴシック W4.ttc",
        "/System/Library/Fonts/ヒラギノ明朝 ProN.ttc",
        "/System/Library/Fonts/CJKSymbolsFallback.ttc",
        "/Library/Fonts/Arial Unicode.ttf",
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "/System/Library/Fonts/SFNS.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
        "/System/Library/Fonts/Supplemental/Times New Roman.ttf",
        "/System/Library/Fonts/Supplemental/Times New Roman Bold.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf",
        "/fonts/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
    ];

    let mut fonts = Vec::new();
    for path in paths {
        if let Ok(data) = std::fs::read(path) {
            if let Ok(font) = FontVec::try_from_vec(data) {
                fonts.push(font);
            }
        }
    }

    FontBook { fonts }
}

fn get_fonts() -> Option<&'static FontBook> {
    let fonts = FONTS.get_or_init(load_fonts);
    if fonts.fonts.is_empty() {
        None
    } else {
        Some(fonts)
    }
}

impl FontBook {
    fn glyph_font_index(&self, ch: char, font_size: f32) -> Option<usize> {
        let fallback = if ch.is_control() { None } else { Some(0) };

        self.fonts
            .iter()
            .enumerate()
            .find_map(|(idx, font)| {
                let scaled = font.as_scaled(font_size);
                if scaled.glyph_id(ch).0 != 0 {
                    Some(idx)
                } else {
                    None
                }
            })
            .or(fallback)
    }

    fn h_advance(&self, ch: char, font_size: f32) -> f32 {
        let Some(idx) = self.glyph_font_index(ch, font_size) else {
            return 0.0;
        };
        let scaled = self.fonts[idx].as_scaled(font_size);
        scaled.h_advance(scaled.glyph_id(ch))
    }

    fn ascent(&self, font_size: f32) -> f32 {
        self.fonts[0].as_scaled(font_size).ascent()
    }
}

pub fn paint(root: &LayoutNode, physical_w: u32, physical_h: u32, scale: f32) -> Pixmap {
    let mut pixmap = Pixmap::new(physical_w, physical_h).expect("failed to create pixmap");
    pixmap.fill(ska_color(Color::WHITE));
    paint_node(root, &mut pixmap, scale);
    pixmap
}

fn paint_node(node: &LayoutNode, pixmap: &mut Pixmap, scale: f32) {
    let x = node.x * scale;
    let y = node.y * scale;
    let w = node.width * scale;
    let h = node.height * scale;
    let bw = node.style.border_width * scale;
    let br = node.style.border_radius * scale;
    let padding_top = node
        .style
        .padding_top
        .resolve_px(node.width, pixmap.width() as f32 / scale)
        * scale;
    let padding_left = node
        .style
        .padding_left
        .resolve_px(node.width, pixmap.width() as f32 / scale)
        * scale;
    let padding_right = node
        .style
        .padding_right
        .resolve_px(node.width, pixmap.width() as f32 / scale)
        * scale;

    if matches!(node.style.visibility, Visibility::Hidden) {
        return;
    }

    if let Some(ref shadow) = node.style.box_shadow {
        if shadow.blur > 0.0 || shadow.offset_x != 0.0 || shadow.offset_y != 0.0 {
            draw_box_shadow(pixmap, x, y, w, h, br, shadow, scale);
        }
    }

    if let Some(ref bg) = node.style.background_color {
        fill_background(
            pixmap,
            x + bw,
            y + bw,
            w - bw * 2.0,
            h - bw * 2.0,
            br,
            bg,
            scale,
        );
    }

    if let Some(ref bg_img) = node.background_image {
        draw_background_image(
            pixmap,
            bg_img,
            node,
            x + bw,
            y + bw,
            w - bw * 2.0,
            h - bw * 2.0,
            scale,
        );
    }

    if bw > 0.0 {
        if let Some(bc) = node.style.border_color {
            draw_border(pixmap, x, y, w, h, bw, br, ska_color(bc));
        }
    }

    if node.style.border_left_width > 0.0 {
        if let Some(color) = node.style.border_left_color.or(node.style.border_color) {
            draw_side_border(
                pixmap,
                x,
                y,
                w,
                h,
                node.style.border_left_width * scale,
                color,
                BorderSide::Left,
            );
        }
    }

    if node.style.border_top_width > 0.0 {
        if let Some(color) = node.style.border_top_color.or(node.style.border_color) {
            draw_side_border(
                pixmap,
                x,
                y,
                w,
                h,
                node.style.border_top_width * scale,
                color,
                BorderSide::Top,
            );
        }
    }

    if node.style.border_right_width > 0.0 {
        if let Some(color) = node.style.border_right_color.or(node.style.border_color) {
            draw_side_border(
                pixmap,
                x,
                y,
                w,
                h,
                node.style.border_right_width * scale,
                color,
                BorderSide::Right,
            );
        }
    }

    if node.style.border_bottom_width > 0.0 {
        if let Some(color) = node.style.border_bottom_color.or(node.style.border_color) {
            draw_side_border(
                pixmap,
                x,
                y,
                w,
                h,
                node.style.border_bottom_width * scale,
                color,
                BorderSide::Bottom,
            );
        }
    }

    if let Some(ref pseudo) = node.pseudo_before {
        paint_pseudo(pseudo, pixmap, x, y, w, h, padding_left, padding_top, scale);
    }

    if node.tag.as_deref() == Some("li") {
        draw_list_marker(
            pixmap,
            x + bw,
            y + padding_top + bw,
            node.style.font_size * scale,
            node.style.color,
        );
    }

    if let Some(ref img) = node.image {
        draw_image(pixmap, img, x, y, w, h);
    }

    if !node.inline_fragments.is_empty() {
        let pseudo_offset = inline_pseudo_offset(node, pixmap.width() as f32 / scale);
        let marker_offset = if node.tag.as_deref() == Some("li") {
            node.style.font_size * scale * 1.2
        } else {
            pseudo_offset
        };
        let mut current_line_y: Option<f32> = None;
        let mut pen_x = x + padding_left + bw + marker_offset;
        for fragment in &node.inline_fragments {
            let line_y = fragment.y * scale;
            if current_line_y != Some(line_y) {
                current_line_y = Some(line_y);
                pen_x = x + padding_left + bw + marker_offset + fragment.x * scale;
            }
            let display_text = match fragment.style.text_transform {
                crate::render::style::TextTransform::Uppercase => fragment.text.to_uppercase(),
                crate::render::style::TextTransform::Lowercase => fragment.text.to_lowercase(),
                _ => fragment.text.clone(),
            };
            let advance = draw_text_run(
                pixmap,
                &display_text,
                pen_x,
                y + padding_top + bw + line_y,
                fragment.style.font_size * scale,
                fragment.style.color,
                fragment.style.letter_spacing * scale,
                fragment
                    .style
                    .line_height
                    .resolve_px(fragment.style.font_size)
                    * scale,
                fragment.style.text_decoration,
                fragment.style.font_weight,
            );
            pen_x += advance;
        }
    } else if let Some(ref text) = node.text {
        let display_text = match node.style.text_transform {
            crate::render::style::TextTransform::Uppercase => text.to_uppercase(),
            crate::render::style::TextTransform::Lowercase => text.to_lowercase(),
            _ => text.clone(),
        };
        let pseudo_offset = node
            .pseudo_before
            .as_ref()
            .map(|pseudo| {
                let pseudo_w = pseudo
                    .width
                    .resolve_px(node.width, pixmap.width() as f32 / scale)
                    * scale;
                if pseudo_w > 0.0 {
                    pseudo_w
                        + node
                            .style
                            .gap
                            .resolve_px(node.width, pixmap.width() as f32 / scale)
                            * scale
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);
        let marker_offset = if node.tag.as_deref() == Some("li") {
            node.style.font_size * scale * 1.2
        } else {
            pseudo_offset
        };
        let text_x = x + padding_left + bw + marker_offset;
        let max_width = (w - padding_left - padding_right - bw * 2.0 - marker_offset)
            .max(node.style.font_size * scale);
        draw_text(
            pixmap,
            &display_text,
            text_x,
            y + padding_top + bw,
            node.style.font_size * scale,
            node.style.color,
            node.style.letter_spacing * scale,
            max_width,
            node.style.text_align,
            node.style.line_height.resolve_px(node.style.font_size) * scale,
            node.style.text_decoration,
            node.style.white_space,
        );
    }

    for child in &node.children {
        paint_node(child, pixmap, scale);
    }

    if let Some(ref pseudo) = node.pseudo_after {
        paint_pseudo(pseudo, pixmap, x, y, w, h, padding_left, padding_top, scale);
    }
}

fn paint_pseudo(
    pseudo: &ComputedStyle,
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    padding_left: f32,
    padding_top: f32,
    scale: f32,
) {
    let Some(ref bg) = pseudo.background_color else {
        return;
    };

    let containing_w = w / scale;
    let viewport_w = pixmap.width() as f32 / scale;
    let pseudo_w = pseudo.width.resolve_px(containing_w, viewport_w) * scale;
    let pseudo_h = pseudo.height.resolve_px(containing_w, viewport_w) * scale;

    if pseudo_w > 0.0 && pseudo_h > 0.0 {
        let px = x + padding_left;
        let py = y + padding_top + (h - padding_top * 2.0 - pseudo_h).max(0.0) / 2.0;
        fill_background(
            pixmap,
            px,
            py,
            pseudo_w,
            pseudo_h,
            pseudo.border_radius * scale,
            bg,
            scale,
        );
    } else {
        fill_background(pixmap, x, y, w, h, pseudo.border_radius * scale, bg, scale);
    }
}

fn inline_pseudo_offset(node: &LayoutNode, viewport_w: f32) -> f32 {
    let Some(ref pseudo) = node.pseudo_before else {
        return 0.0;
    };
    let pseudo_w = pseudo.width.resolve_px(node.width, viewport_w);
    if pseudo_w <= 0.0 {
        return 0.0;
    }
    pseudo_w + node.style.gap.resolve_px(node.width, viewport_w)
}

fn draw_text(
    pixmap: &mut Pixmap,
    text: &str,
    x: f32,
    y: f32,
    font_size: f32,
    color: Color,
    letter_spacing: f32,
    max_width: f32,
    text_align: TextAlign,
    line_height_px: f32,
    text_decoration: TextDecoration,
    white_space: WhiteSpace,
) {
    let Some(fonts) = get_fonts() else {
        draw_text_placeholder(pixmap, text, x, y, font_size, color);
        return;
    };

    let ascent = fonts.ascent(font_size);
    let descent = font_size - ascent;
    let pw = pixmap.width();
    let ph = pixmap.height();
    let space_advance = fonts.h_advance(' ', font_size) + letter_spacing;

    let mut lines: Vec<(f32, String)> = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0.0f32;

    if matches!(white_space, WhiteSpace::Nowrap) {
        current_width = measure_word_width(fonts, text, font_size, letter_spacing);
        current_line.push_str(text);
    } else if matches!(white_space, WhiteSpace::Pre) {
        for line in text.split('\n') {
            lines.push((
                measure_word_width(fonts, line, font_size, letter_spacing),
                line.to_string(),
            ));
        }
    } else {
        for hard_line in text.split('\n') {
            for word in hard_line.split(' ') {
                let word_width = measure_word_width(fonts, word, font_size, letter_spacing);
                let sep = if !current_line.is_empty() {
                    space_advance
                } else {
                    0.0
                };

                if !current_line.is_empty() && current_width + sep + word_width > max_width {
                    lines.push((current_width, std::mem::take(&mut current_line)));
                    current_width = 0.0;
                } else if sep > 0.0 {
                    current_line.push(' ');
                    current_width += sep;
                }

                if word_width > max_width && max_width > font_size {
                    for ch in word.chars() {
                        let ch_width = fonts.h_advance(ch, font_size) + letter_spacing;
                        if !current_line.is_empty() && current_width + ch_width > max_width {
                            lines.push((current_width, std::mem::take(&mut current_line)));
                            current_width = 0.0;
                        }
                        current_line.push(ch);
                        current_width += ch_width;
                    }
                } else {
                    current_line.push_str(word);
                    current_width += word_width;
                }
            }
            lines.push((current_width, std::mem::take(&mut current_line)));
            current_width = 0.0;
        }
        if lines.last().is_some_and(|(_, line)| line.is_empty()) {
            lines.pop();
        }
    }
    if !current_line.is_empty() {
        lines.push((current_width, current_line));
    }
    if lines.is_empty() {
        lines.push((0.0, String::new()));
    }

    let mut pen_y = y + ascent;

    for (line_w, line_text) in &lines {
        let align_offset = match text_align {
            TextAlign::Center => ((max_width - line_w) / 2.0).max(0.0),
            TextAlign::Right => (max_width - line_w).max(0.0),
            _ => 0.0,
        };
        let mut pen_x = x + align_offset;

        for ch in line_text.chars() {
            let Some(font_idx) = fonts.glyph_font_index(ch, font_size) else {
                continue;
            };
            let scaled = fonts.fonts[font_idx].as_scaled(font_size);
            let glyph_id = scaled.glyph_id(ch);
            let advance = scaled.h_advance(glyph_id);
            let glyph: Glyph = glyph_id.with_scale_and_position(font_size, (pen_x, pen_y));
            if let Some(outlined) = scaled.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                let px0 = bounds.min.x;
                let py0 = bounds.min.y;
                outlined.draw(|gx: u32, gy: u32, coverage: f32| {
                    let px = px0 + gx as f32;
                    let py = py0 + gy as f32;
                    if px >= 0.0 && py >= 0.0 && px < pw as f32 && py < ph as f32 {
                        let ix = px as usize;
                        let iy = py as usize;
                        let idx = (iy * pw as usize + ix) * 4;
                        let alpha = (coverage * color.a as f32) as u8;
                        let pixels = pixmap.data_mut();
                        pixels[idx] = blend(pixels[idx], color.r, alpha);
                        pixels[idx + 1] = blend(pixels[idx + 1], color.g, alpha);
                        pixels[idx + 2] = blend(pixels[idx + 2], color.b, alpha);
                        pixels[idx + 3] = 255;
                    }
                });
            }
            pen_x += advance + letter_spacing;
        }

        match text_decoration {
            TextDecoration::Underline => {
                let uy = pen_y + descent * 0.3;
                draw_horizontal_line(pixmap, x + align_offset, uy, *line_w, color);
            }
            TextDecoration::LineThrough => {
                let sty = pen_y - font_size * 0.25;
                draw_horizontal_line(pixmap, x + align_offset, sty, *line_w, color);
            }
            TextDecoration::Overline => {
                let oy = pen_y - ascent;
                draw_horizontal_line(pixmap, x + align_offset, oy, *line_w, color);
            }
            TextDecoration::None => {}
        }

        pen_y += line_height_px;
    }
}

fn draw_text_run(
    pixmap: &mut Pixmap,
    text: &str,
    x: f32,
    y: f32,
    font_size: f32,
    color: Color,
    letter_spacing: f32,
    line_height_px: f32,
    text_decoration: TextDecoration,
    font_weight: u32,
) -> f32 {
    let Some(fonts) = get_fonts() else {
        draw_text_placeholder(pixmap, text, x, y, font_size, color);
        return font_size * 0.6 * text.chars().count() as f32;
    };

    let ascent = fonts.ascent(font_size);
    let descent = font_size - ascent;
    let pw = pixmap.width();
    let ph = pixmap.height();
    let mut pen_x = x;
    let pen_y = y + ascent;

    for ch in text.chars() {
        let Some(font_idx) = fonts.glyph_font_index(ch, font_size) else {
            continue;
        };
        let scaled = fonts.fonts[font_idx].as_scaled(font_size);
        let glyph_id = scaled.glyph_id(ch);
        let advance = scaled.h_advance(glyph_id);
        let glyph: Glyph = glyph_id.with_scale_and_position(font_size, (pen_x, pen_y));
        if let Some(outlined) = scaled.outline_glyph(glyph) {
            draw_outlined_glyph(pixmap, &outlined, color, 0.0, 0.0, pw, ph);
            if font_weight >= 600 {
                draw_outlined_glyph(pixmap, &outlined, color, 0.45, 0.0, pw, ph);
            }
        }
        pen_x += advance + letter_spacing;
    }

    let width = pen_x - x;
    match text_decoration {
        TextDecoration::Underline => {
            draw_horizontal_line(pixmap, x, pen_y + descent * 0.3, width, color)
        }
        TextDecoration::LineThrough => {
            draw_horizontal_line(pixmap, x, pen_y - font_size * 0.25, width, color)
        }
        TextDecoration::Overline => draw_horizontal_line(pixmap, x, pen_y - ascent, width, color),
        TextDecoration::None => {}
    }

    let _ = line_height_px;
    width
}

fn draw_outlined_glyph(
    pixmap: &mut Pixmap,
    outlined: &OutlinedGlyph,
    color: Color,
    offset_x: f32,
    offset_y: f32,
    pixmap_width: u32,
    pixmap_height: u32,
) {
    let bounds = outlined.px_bounds();
    let px0 = bounds.min.x + offset_x;
    let py0 = bounds.min.y + offset_y;
    outlined.draw(|gx: u32, gy: u32, coverage: f32| {
        let px = px0 + gx as f32;
        let py = py0 + gy as f32;
        if px >= 0.0 && py >= 0.0 && px < pixmap_width as f32 && py < pixmap_height as f32 {
            let ix = px as usize;
            let iy = py as usize;
            let idx = (iy * pixmap_width as usize + ix) * 4;
            let alpha = (coverage * color.a as f32) as u8;
            let pixels = pixmap.data_mut();
            pixels[idx] = blend(pixels[idx], color.r, alpha);
            pixels[idx + 1] = blend(pixels[idx + 1], color.g, alpha);
            pixels[idx + 2] = blend(pixels[idx + 2], color.b, alpha);
            pixels[idx + 3] = 255;
        }
    });
}

fn draw_horizontal_line(pixmap: &mut Pixmap, x: f32, y: f32, width: f32, color: Color) {
    if width <= 0.0 || y < 0.0 || y as u32 >= pixmap.height() {
        return;
    }
    let iy = y.round() as u32;
    let start = x.max(0.0) as u32;
    let end = (x + width).min(pixmap.width() as f32) as u32;
    let pw = pixmap.width() as usize;
    let pixels = pixmap.data_mut();
    for ix in start..end {
        let idx = (iy as usize * pw + ix as usize) * 4;
        let alpha = color.a as u32;
        pixels[idx] = blend(pixels[idx], color.r, alpha as u8);
        pixels[idx + 1] = blend(pixels[idx + 1], color.g, alpha as u8);
        pixels[idx + 2] = blend(pixels[idx + 2], color.b, alpha as u8);
        pixels[idx + 3] = 255;
    }
}

fn measure_word_width(fonts: &FontBook, word: &str, font_size: f32, letter_spacing: f32) -> f32 {
    word.chars()
        .map(|ch| fonts.h_advance(ch, font_size) + letter_spacing)
        .sum()
}

fn blend(dst: u8, src: u8, alpha: u8) -> u8 {
    let a = alpha as f32 / 255.0;
    (src as f32 * a + dst as f32 * (1.0 - a)) as u8
}

fn draw_list_marker(pixmap: &mut Pixmap, x: f32, y: f32, font_size: f32, color: Color) {
    let size = (font_size * 0.28).max(4.0);
    fill_rounded_rect(
        pixmap,
        x + font_size * 0.35,
        y + font_size * 0.5,
        size,
        size,
        size / 2.0,
        ska_color(color),
    );
}

fn draw_text_placeholder(
    pixmap: &mut Pixmap,
    text: &str,
    x: f32,
    y: f32,
    font_size: f32,
    color: Color,
) {
    let mut paint = Paint::default();
    paint.set_color(ska_color(color));
    paint.anti_alias = true;
    let rect = Rect::from_xywh(x, y, font_size * 0.6 * text.len() as f32, font_size)
        .unwrap_or_else(|| Rect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap());
    pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
}

fn fill_background(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    bg: &Background,
    _scale: f32,
) {
    match bg {
        Background::Solid(c) => {
            fill_rounded_rect(pixmap, x, y, w, h, r, ska_color(*c));
        }
        Background::Linear { angle_deg, stops } => {
            fill_linear_gradient(pixmap, x, y, w, h, r, *angle_deg, stops);
        }
        Background::Radial {
            center_x,
            center_y,
            stops,
        } => {
            fill_radial_gradient(pixmap, x, y, w, h, r, *center_x, *center_y, stops);
        }
        Background::RepeatingLinear {
            angle_deg,
            color,
            stripe_width,
            period,
        } => {
            fill_repeating_linear_gradient(
                pixmap,
                x,
                y,
                w,
                h,
                r,
                *angle_deg,
                *color,
                *stripe_width,
                *period,
                _scale,
            );
        }
        Background::Layers(layers) => {
            for layer in layers.iter().rev() {
                fill_background(pixmap, x, y, w, h, r, layer, _scale);
            }
        }
    }
}

fn fill_repeating_linear_gradient(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    angle_deg: f32,
    color: Color,
    stripe_width: f32,
    period: f32,
    scale: f32,
) {
    let stripe_width = stripe_width * scale;
    let period = period * scale;
    let (dx, dy) = css_gradient_vector(angle_deg);
    let extent = (w * dx.abs() + h * dy.abs()).max(1.0);

    fill_pixels(pixmap, x, y, w, h, r, |px, py| {
        let proj = (px - x) * dx + (py - y) * dy + extent;
        let pos = proj.rem_euclid(period.max(stripe_width + 1.0));
        if pos <= stripe_width {
            Some(color)
        } else {
            None
        }
    });
}

fn fill_linear_gradient(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    angle_deg: f32,
    stops: &[(Color, f32)],
) {
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    let (dx, dy) = css_gradient_vector(angle_deg);
    let extent = ((w * dx.abs() + h * dy.abs()) / 2.0).max(1.0);

    fill_pixels(pixmap, x, y, w, h, r, |px, py| {
        let proj = (px - cx) * dx + (py - cy) * dy;
        Some(interpolate_stops(stops, (proj / extent + 1.0) * 0.5))
    });
}

fn fill_radial_gradient(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    center_x: f32,
    center_y: f32,
    stops: &[(Color, f32)],
) {
    let cx = x + w * center_x;
    let cy = y + h * center_y;
    let corners = [
        ((x - cx).hypot(y - cy)),
        ((x + w - cx).hypot(y - cy)),
        ((x - cx).hypot(y + h - cy)),
        ((x + w - cx).hypot(y + h - cy)),
    ];
    let radius = corners.into_iter().fold(1.0f32, f32::max);

    fill_pixels(pixmap, x, y, w, h, r, |px, py| {
        Some(interpolate_stops(stops, (px - cx).hypot(py - cy) / radius))
    });
}

fn css_gradient_vector(angle_deg: f32) -> (f32, f32) {
    let radians = angle_deg.to_radians();
    (radians.sin(), -radians.cos())
}

fn fill_pixels(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    mut color_at: impl FnMut(f32, f32) -> Option<Color>,
) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }

    let min_x = x.max(0.0).floor() as i32;
    let min_y = y.max(0.0).floor() as i32;
    let max_x = (x + w).min(pixmap.width() as f32).ceil() as i32;
    let max_y = (y + h).min(pixmap.height() as f32).ceil() as i32;
    let pw = pixmap.width() as usize;

    for py in min_y..max_y {
        for px in min_x..max_x {
            let fx = px as f32 + 0.5;
            let fy = py as f32 + 0.5;
            if !inside_rounded_rect(fx, fy, x, y, w, h, r) {
                continue;
            }
            let Some(color) = color_at(fx, fy) else {
                continue;
            };
            blend_pixel(pixmap.data_mut(), pw, px as usize, py as usize, color);
        }
    }
}

fn inside_rounded_rect(px: f32, py: f32, x: f32, y: f32, w: f32, h: f32, r: f32) -> bool {
    if r <= 0.0 {
        return px >= x && py >= y && px <= x + w && py <= y + h;
    }

    let r = r.min(w / 2.0).min(h / 2.0);
    let inner_left = x + r;
    let inner_right = x + w - r;
    let inner_top = y + r;
    let inner_bottom = y + h - r;

    if (px >= inner_left && px <= inner_right) || (py >= inner_top && py <= inner_bottom) {
        return true;
    }

    let cx = if px < inner_left {
        inner_left
    } else {
        inner_right
    };
    let cy = if py < inner_top {
        inner_top
    } else {
        inner_bottom
    };
    (px - cx).hypot(py - cy) <= r
}

fn blend_pixel(data: &mut [u8], width: usize, x: usize, y: usize, color: Color) {
    let idx = (y * width + x) * 4;
    let alpha = color.a as f32 / 255.0;
    data[idx] = (color.r as f32 * alpha + data[idx] as f32 * (1.0 - alpha)) as u8;
    data[idx + 1] = (color.g as f32 * alpha + data[idx + 1] as f32 * (1.0 - alpha)) as u8;
    data[idx + 2] = (color.b as f32 * alpha + data[idx + 2] as f32 * (1.0 - alpha)) as u8;
    data[idx + 3] = 255;
}

fn interpolate_stops(stops: &[(Color, f32)], t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    if stops.is_empty() {
        return Color::WHITE;
    }
    if stops.len() == 1 || t <= stops[0].1 {
        return stops[0].0;
    }
    if t >= stops[stops.len() - 1].1 {
        return stops[stops.len() - 1].0;
    }

    for i in 0..stops.len() - 1 {
        let (c1, p1) = stops[i];
        let (c2, p2) = stops[i + 1];
        if t >= p1 && t <= p2 {
            let local_t = if p2 - p1 > 0.0 {
                (t - p1) / (p2 - p1)
            } else {
                0.0
            };
            return lerp_color(c1, c2, local_t);
        }
    }

    stops[stops.len() - 1].0
}

fn lerp_color(c1: Color, c2: Color, t: f32) -> Color {
    Color {
        r: (c1.r as f32 + (c2.r as f32 - c1.r as f32) * t) as u8,
        g: (c1.g as f32 + (c2.g as f32 - c1.g as f32) * t) as u8,
        b: (c1.b as f32 + (c2.b as f32 - c1.b as f32) * t) as u8,
        a: (c1.a as f32 + (c2.a as f32 - c1.a as f32) * t) as u8,
    }
}

fn draw_image(
    pixmap: &mut Pixmap,
    img: &crate::render::layout::LayoutImage,
    dst_x: f32,
    dst_y: f32,
    dst_w: f32,
    dst_h: f32,
) {
    if img.width == 0 || img.height == 0 || img.rgba.is_empty() {
        return;
    }

    let src_w = img.width as f32;
    let src_h = img.height as f32;
    let dst_iw = dst_w.round() as u32;
    let dst_ih = dst_h.round() as u32;
    if dst_iw == 0 || dst_ih == 0 {
        return;
    }

    let scale_x = src_w / dst_w;
    let scale_y = src_h / dst_h;
    let pixmap_w = pixmap.width();

    for dy in 0..dst_ih {
        let sy = (dy as f32 * scale_y) as u32;
        if sy >= img.height {
            break;
        }
        for dx in 0..dst_iw {
            let sx = (dx as f32 * scale_x) as u32;
            if sx >= img.width {
                break;
            }
            let src_idx = ((sy * img.width + sx) * 4) as usize;
            if src_idx + 3 >= img.rgba.len() {
                break;
            }
            let px = (dst_x.round() as i64 + dx as i64) as u32;
            let py = (dst_y.round() as i64 + dy as i64) as u32;
            if px >= pixmap_w || py >= pixmap.height() {
                continue;
            }
            let dst_idx = ((py * pixmap_w + px) * 4) as usize;
            let data = pixmap.data_mut();
            let r = img.rgba[src_idx];
            let g = img.rgba[src_idx + 1];
            let b = img.rgba[src_idx + 2];
            let a = img.rgba[src_idx + 3];
            if a == 255 {
                data[dst_idx] = r;
                data[dst_idx + 1] = g;
                data[dst_idx + 2] = b;
                data[dst_idx + 3] = 255;
            } else if a > 0 {
                let alpha = a as f32 / 255.0;
                data[dst_idx] = (r as f32 * alpha + data[dst_idx] as f32 * (1.0 - alpha)) as u8;
                data[dst_idx + 1] =
                    (g as f32 * alpha + data[dst_idx + 1] as f32 * (1.0 - alpha)) as u8;
                data[dst_idx + 2] =
                    (b as f32 * alpha + data[dst_idx + 2] as f32 * (1.0 - alpha)) as u8;
                data[dst_idx + 3] = 255;
            }
        }
    }
}

fn draw_background_image(
    pixmap: &mut Pixmap,
    img: &crate::render::layout::LayoutImage,
    node: &LayoutNode,
    area_x: f32,
    area_y: f32,
    area_w: f32,
    area_h: f32,
    scale: f32,
) {
    if img.width == 0 || img.height == 0 || img.rgba.is_empty() || area_w <= 0.0 || area_h <= 0.0 {
        return;
    }

    let image_w = img.width as f32 * scale;
    let image_h = img.height as f32 * scale;
    let logical_area_w = area_w / scale;
    let logical_area_h = area_h / scale;
    let viewport_w = pixmap.width() as f32 / scale;
    let offset_x = resolve_background_offset(
        node.style.background_position_x,
        logical_area_w,
        img.width as f32,
        viewport_w,
    ) * scale;
    let offset_y = resolve_background_offset(
        node.style.background_position_y,
        logical_area_h,
        img.height as f32,
        viewport_w,
    ) * scale;

    let repeat_x = node.style.background_repeat_x;
    let repeat_y = node.style.background_repeat_y;

    let start_x = if repeat_x {
        area_x + offset_x.rem_euclid(image_w) - image_w
    } else {
        area_x + offset_x
    };
    let start_y = if repeat_y {
        area_y + offset_y.rem_euclid(image_h) - image_h
    } else {
        area_y + offset_y
    };

    let mut ty = start_y;
    loop {
        let mut tx = start_x;
        loop {
            draw_image_clipped(
                pixmap, img, tx, ty, image_w, image_h, area_x, area_y, area_w, area_h,
            );
            if !repeat_x {
                break;
            }
            tx += image_w;
            if tx >= area_x + area_w {
                break;
            }
        }
        if !repeat_y {
            break;
        }
        ty += image_h;
        if ty >= area_y + area_h {
            break;
        }
    }
}

fn resolve_background_offset(
    axis: BackgroundPositionAxis,
    area: f32,
    image: f32,
    viewport_w: f32,
) -> f32 {
    match axis {
        BackgroundPositionAxis::Start(length) => length.resolve_px(area, viewport_w),
        BackgroundPositionAxis::Center(length) => {
            (area - image) * 0.5 + length.resolve_px(area, viewport_w)
        }
        BackgroundPositionAxis::End(length) => (area - image) + length.resolve_px(area, viewport_w),
    }
}

fn draw_image_clipped(
    pixmap: &mut Pixmap,
    img: &crate::render::layout::LayoutImage,
    dst_x: f32,
    dst_y: f32,
    dst_w: f32,
    dst_h: f32,
    clip_x: f32,
    clip_y: f32,
    clip_w: f32,
    clip_h: f32,
) {
    if img.width == 0 || img.height == 0 || img.rgba.is_empty() || dst_w <= 0.0 || dst_h <= 0.0 {
        return;
    }

    let src_w = img.width as f32;
    let src_h = img.height as f32;
    let left = dst_x.round() as i32;
    let top = dst_y.round() as i32;
    let right = (dst_x + dst_w).round() as i32;
    let bottom = (dst_y + dst_h).round() as i32;
    let clip_left = clip_x.round() as i32;
    let clip_top = clip_y.round() as i32;
    let clip_right = (clip_x + clip_w).round() as i32;
    let clip_bottom = (clip_y + clip_h).round() as i32;
    if right <= left || bottom <= top {
        return;
    }

    let scale_x = src_w / dst_w;
    let scale_y = src_h / dst_h;
    let pixmap_w = pixmap.width() as i32;
    let pixmap_h = pixmap.height() as i32;
    let draw_left = left.max(0).max(clip_left);
    let draw_top = top.max(0).max(clip_top);
    let draw_right = right.min(pixmap_w).min(clip_right);
    let draw_bottom = bottom.min(pixmap_h).min(clip_bottom);
    if draw_right <= draw_left || draw_bottom <= draw_top {
        return;
    }

    for py in draw_top..draw_bottom {
        let sy = ((py - top) as f32 * scale_y) as u32;
        if sy >= img.height {
            continue;
        }
        for px in draw_left..draw_right {
            let sx = ((px - left) as f32 * scale_x) as u32;
            if sx >= img.width {
                continue;
            }
            let src_idx = ((sy * img.width + sx) * 4) as usize;
            if src_idx + 3 >= img.rgba.len() {
                continue;
            }
            let dst_idx = (((py as u32) * pixmap.width() + px as u32) * 4) as usize;
            let data = pixmap.data_mut();
            let r = img.rgba[src_idx];
            let g = img.rgba[src_idx + 1];
            let b = img.rgba[src_idx + 2];
            let a = img.rgba[src_idx + 3];
            if a == 255 {
                data[dst_idx] = r;
                data[dst_idx + 1] = g;
                data[dst_idx + 2] = b;
                data[dst_idx + 3] = 255;
            } else if a > 0 {
                let alpha = a as f32 / 255.0;
                data[dst_idx] = (r as f32 * alpha + data[dst_idx] as f32 * (1.0 - alpha)) as u8;
                data[dst_idx + 1] =
                    (g as f32 * alpha + data[dst_idx + 1] as f32 * (1.0 - alpha)) as u8;
                data[dst_idx + 2] =
                    (b as f32 * alpha + data[dst_idx + 2] as f32 * (1.0 - alpha)) as u8;
                data[dst_idx + 3] = 255;
            }
        }
    }
}

fn fill_rounded_rect(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, color: SkColor) {
    if r <= 0.0 {
        if let Some(rect) = Rect::from_xywh(x, y, w, h) {
            let mut paint = Paint::default();
            paint.set_color(color);
            paint.anti_alias = true;
            pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
        }
        return;
    }

    let r = r.min(w / 2.0).min(h / 2.0);
    if let Some(path) = build_rounded_rect_path(x, y, w, h, r) {
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        pixmap.fill_path(
            &path,
            &paint,
            tiny_skia::FillRule::Winding,
            tiny_skia::Transform::identity(),
            None,
        );
    }
}

fn draw_box_shadow(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    shadow: &BoxShadow,
    scale: f32,
) {
    let sx = x + shadow.offset_x * scale;
    let sy = y + shadow.offset_y * scale;
    let sw = w;
    let sh = h;
    let blur = shadow.blur * scale;
    let alpha = shadow.color.a as f32 / 255.0;
    let base_color = SkColor::from_rgba(
        shadow.color.r as f32 / 255.0,
        shadow.color.g as f32 / 255.0,
        shadow.color.b as f32 / 255.0,
        alpha * 0.3,
    )
    .unwrap_or(SkColor::BLACK);

    let layers = 6;
    for i in 0..layers {
        let t = i as f32 / layers as f32;
        let expand = blur * t;
        let layer_alpha = alpha * (1.0 - t) * 0.15;
        let color = SkColor::from_rgba(
            shadow.color.r as f32 / 255.0,
            shadow.color.g as f32 / 255.0,
            shadow.color.b as f32 / 255.0,
            layer_alpha,
        )
        .unwrap_or(base_color);

        fill_rounded_rect(
            pixmap,
            sx - expand,
            sy - expand,
            sw + expand * 2.0,
            sh + expand * 2.0,
            r + expand,
            color,
        );
    }
}

fn draw_border(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    bw: f32,
    r: f32,
    color: SkColor,
) {
    let r = r.min(w / 2.0).min(h / 2.0);
    if r <= 0.0 {
        let sides = [
            Rect::from_xywh(x, y, w, bw),
            Rect::from_xywh(x, y + h - bw, w, bw),
            Rect::from_xywh(x, y, bw, h),
            Rect::from_xywh(x + w - bw, y, bw, h),
        ];
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        for rect in sides.iter().flatten() {
            pixmap.fill_rect(*rect, &paint, tiny_skia::Transform::identity(), None);
        }
        return;
    }

    let outer = build_rounded_rect_path(x, y, w, h, r);
    let inner = build_rounded_rect_path(
        x + bw,
        y + bw,
        w - bw * 2.0,
        h - bw * 2.0,
        (r - bw).max(0.0),
    );
    if let (Some(outer), Some(inner)) = (outer, inner) {
        let mut pb = PathBuilder::new();
        pb.push_path(&outer);
        pb.push_path(&inner);
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color(color);
            paint.anti_alias = true;
            pixmap.fill_path(
                &path,
                &paint,
                tiny_skia::FillRule::EvenOdd,
                tiny_skia::Transform::identity(),
                None,
            );
        }
    }
}

enum BorderSide {
    Top,
    Right,
    Bottom,
    Left,
}

fn draw_side_border(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    width: f32,
    color: Color,
    side: BorderSide,
) {
    if width <= 0.0 || w <= 0.0 || h <= 0.0 {
        return;
    }

    let (rx, ry, rw, rh) = match side {
        BorderSide::Top => (x, y, w, width),
        BorderSide::Bottom => (x, y + h - width, w, width),
        BorderSide::Left => (x, y, width, h),
        BorderSide::Right => (x + w - width, y, width, h),
    };

    let min_x = rx.max(0.0).floor() as i32;
    let min_y = ry.max(0.0).floor() as i32;
    let max_x = (rx + rw).min(pixmap.width() as f32).ceil() as i32;
    let max_y = (ry + rh).min(pixmap.height() as f32).ceil() as i32;
    let pw = pixmap.width() as usize;

    for py in min_y..max_y {
        for px in min_x..max_x {
            blend_pixel(pixmap.data_mut(), pw, px as usize, py as usize, color);
        }
    }
}

fn build_rounded_rect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r, y, x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r, x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r, y + h, x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r, x, y, x + r, y);
    pb.close();
    pb.finish()
}

fn ska_color(c: Color) -> SkColor {
    SkColor::from_rgba(
        c.r as f32 / 255.0,
        c.g as f32 / 255.0,
        c.b as f32 / 255.0,
        c.a as f32 / 255.0,
    )
    .expect("invalid color component")
}
