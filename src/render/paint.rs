use std::sync::OnceLock;

use ab_glyph::{Font, FontVec, Glyph, ScaleFont};
use tiny_skia::{Color as SkColor, Paint, PathBuilder, Pixmap, Rect};

use crate::render::layout::LayoutNode;
use crate::render::style::{Background, BoxShadow, Color, ComputedStyle};

static FONT: OnceLock<Option<FontVec>> = OnceLock::new();

fn load_font() -> Option<FontVec> {
    let paths = [
        "/System/Library/Fonts/Supplemental/Times New Roman.ttf",
        "/System/Library/Fonts/Supplemental/Times New Roman Bold.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf",
        "/fonts/Mplus1-Regular.ttf",
        "/fonts/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
    ];

    for path in paths {
        if let Ok(data) = std::fs::read(path) {
            if let Ok(font) = FontVec::try_from_vec(data) {
                return Some(font);
            }
        }
    }
    None
}

fn get_font() -> Option<&'static FontVec> {
    FONT.get_or_init(load_font).as_ref()
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

    if bw > 0.0 {
        if let Some(bc) = node.style.border_color {
            draw_border(pixmap, x, y, w, h, bw, br, ska_color(bc));
        }
    }

    if node.style.border_left_width > 0.0 {
        if let Some(color) = node.style.border_left_color.or(node.style.border_color) {
            draw_left_border(
                pixmap,
                x,
                y,
                h,
                node.style.border_left_width * scale,
                br,
                ska_color(color),
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

    if let Some(ref text) = node.text {
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

fn draw_text(
    pixmap: &mut Pixmap,
    text: &str,
    x: f32,
    y: f32,
    font_size: f32,
    color: Color,
    letter_spacing: f32,
    max_width: f32,
) {
    let Some(font) = get_font() else {
        draw_text_placeholder(pixmap, text, x, y, font_size, color);
        return;
    };

    let scaled = font.as_scaled(font_size);
    let ascent = scaled.ascent();
    let line_height = font_size * 1.2;

    let pw = pixmap.width();
    let ph = pixmap.height();

    let mut pen_x = x;
    let mut pen_y = y + ascent;

    let space_advance = scaled.h_advance(scaled.glyph_id(' ')) + letter_spacing;

    for word in text.split(' ') {
        let word_width = measure_word_width(&scaled, word, letter_spacing);
        let sep = if pen_x > x { space_advance } else { 0.0 };
        if pen_x > x && pen_x + sep + word_width > x + max_width {
            pen_x = x;
            pen_y += line_height;
        }

        if pen_x > x {
            pen_x += space_advance;
        }

        for ch in word.chars() {
            let glyph_id = scaled.glyph_id(ch);
            let advance = scaled.h_advance(glyph_id);
            let glyph: Glyph = glyph_id.with_scale_and_position(font_size, (pen_x, pen_y));
            if let Some(outlined) = scaled.outline_glyph(glyph) {
                let bounds = outlined.bounds();
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

            if pen_x > x + max_width {
                pen_x = x;
                pen_y += line_height;
            }
        }
    }
}

fn measure_word_width<F: Font>(font: &impl ScaleFont<F>, word: &str, letter_spacing: f32) -> f32 {
    word.chars()
        .map(|ch| font.h_advance(font.glyph_id(ch)) + letter_spacing)
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

fn draw_left_border(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    h: f32,
    width: f32,
    radius: f32,
    color: SkColor,
) {
    fill_rounded_rect(pixmap, x, y, width, h, radius.min(width / 2.0), color);
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
