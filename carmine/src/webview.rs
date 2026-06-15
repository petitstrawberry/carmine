use std::cell::RefCell;
use std::rc::Rc;

use scarlet_ui::event::{Event, InputEvent, KeyCode, KeyEvent, MouseEvent};
use scarlet_ui::graphics;
use scarlet_ui::prelude::*;
use scarlet_ui::{CanvasView, ViewExt};

use carmine_core::render::{RenderOptions, RenderPipeline};

use crate::paint_signal::PaintSignal;

pub struct WebView {
    pipeline: Rc<RefCell<RenderPipeline>>,
    scroll_y: Rc<RefCell<f32>>,
    content_height: Rc<RefCell<f32>>,
    paint_signal: Rc<PaintSignal>,
    width: f32,
    height: f32,
}

impl WebView {
    pub fn new(
        html: &str,
        width: u32,
        height: u32,
        options: RenderOptions,
        paint_signal: Rc<PaintSignal>,
    ) -> Self {
        let pipeline = RenderPipeline::with_options(html, width, height, options);
        Self {
            pipeline: Rc::new(RefCell::new(pipeline)),
            scroll_y: Rc::new(RefCell::new(0.0)),
            content_height: Rc::new(RefCell::new(height as f32)),
            paint_signal,
            width: width as f32,
            height: height as f32,
        }
    }
}

impl Clone for WebView {
    fn clone(&self) -> Self {
        Self {
            pipeline: Rc::clone(&self.pipeline),
            scroll_y: Rc::clone(&self.scroll_y),
            content_height: Rc::clone(&self.content_height),
            paint_signal: Rc::clone(&self.paint_signal),
            width: self.width,
            height: self.height,
        }
    }
}

impl View for WebView {
    fn create_element(&self) -> Box<dyn Element> {
        let pipeline = Rc::clone(&self.pipeline);
        let scroll_y_canvas = Rc::clone(&self.scroll_y);
        let content_height_canvas = Rc::clone(&self.content_height);

        let scroll_y_event = Rc::clone(&self.scroll_y);
        let content_height_event = Rc::clone(&self.content_height);
        let paint_signal_event = Rc::clone(&self.paint_signal);

        let scroll_y_key = Rc::clone(&self.scroll_y);
        let content_height_key = Rc::clone(&self.content_height);
        let paint_signal_key = Rc::clone(&self.paint_signal);
        let viewport_h = self.height;
        CanvasView::new(
            self.width,
            self.height,
            Rc::new(move |buffer, width, height| {
                let scale = graphics::current_scale_milli().max(1) as f32 / 1000.0;

                let mut pipe = pipeline.borrow_mut();
                let content_h_logical = pipe.content_height(width, height, scale);
                let pixmap = pipe.render_to_pixmap(width, height, scale);
                *content_height_canvas.borrow_mut() = content_h_logical;

                let viewport_h_logical = (height as f32) / scale;
                {
                    let mut sy = scroll_y_canvas.borrow_mut();
                    let max_scroll = (content_h_logical - viewport_h_logical).max(0.0);
                    *sy = sy.clamp(0.0, max_scroll);
                }

                let sy = ((*scroll_y_canvas.borrow()) * scale).round() as u32;
                let src = pixmap.data();
                let src_w = pixmap.width() as usize;
                let src_stride = src_w * 4;

                let dst_w = width as usize;
                let full_h = pixmap.height() as u32;
                let visible_h = height.min(full_h.saturating_sub(sy));

                for y in 0..visible_h as usize {
                    let src_off = (sy as usize + y) * src_stride;
                    let dst_off = y * dst_w * 4;
                    let copy_bytes = dst_w * 4;

                    if src_off + copy_bytes <= src.len() && dst_off + copy_bytes <= buffer.len() {
                        let src_row = &src[src_off..src_off + copy_bytes];
                        let dst_row = &mut buffer[dst_off..dst_off + copy_bytes];
                        for (s, d) in src_row.chunks_exact(4).zip(dst_row.chunks_exact_mut(4)) {
                            d[0] = s[2];
                            d[1] = s[1];
                            d[2] = s[0];
                            d[3] = s[3];
                        }
                    }
                }

                if (visible_h as usize) < height as usize {
                    let fill_start = visible_h as usize * dst_w * 4;
                    for b in &mut buffer[fill_start..] {
                        *b = 0;
                    }
                }
            }),
        )
        .on_event(move |event| {
            let max_scroll = || -> f32 { (*content_height_event.borrow() - viewport_h).max(0.0) };

            let changed = match event {
                Event::Mouse(MouseEvent::Wheel { delta_y, .. }) => {
                    let mut sy = scroll_y_event.borrow_mut();
                    let new_sy = (*sy + *delta_y as f32).clamp(0.0, max_scroll());
                    if (new_sy - *sy).abs() > 0.5 {
                        *sy = new_sy;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            };

            if changed {
                paint_signal_event.notify();
            }
            changed
        })
        .on_key(move |key_event| {
            let max_scroll = || -> f32 { (*content_height_key.borrow() - viewport_h).max(0.0) };
            let step = match key_event {
                KeyEvent::Pressed {
                    keycode: KeyCode::PageDown | KeyCode::PageUp,
                } => viewport_h * 0.8,
                _ => 40.0,
            };
            let changed = match key_event {
                KeyEvent::Pressed {
                    keycode: KeyCode::Down | KeyCode::PageDown,
                } => {
                    let mut sy = scroll_y_key.borrow_mut();
                    let new_sy = (*sy + step).clamp(0.0, max_scroll());
                    if (new_sy - *sy).abs() > 0.5 {
                        *sy = new_sy;
                        true
                    } else {
                        false
                    }
                }
                KeyEvent::Pressed {
                    keycode: KeyCode::Up | KeyCode::PageUp,
                } => {
                    let mut sy = scroll_y_key.borrow_mut();
                    let new_sy = (*sy - step).clamp(0.0, max_scroll());
                    if (new_sy - *sy).abs() > 0.5 {
                        *sy = new_sy;
                        true
                    } else {
                        false
                    }
                }
                KeyEvent::Pressed {
                    keycode: KeyCode::Home,
                } => {
                    let mut sy = scroll_y_key.borrow_mut();
                    if *sy > 0.0 {
                        *sy = 0.0;
                        true
                    } else {
                        false
                    }
                }
                KeyEvent::Pressed {
                    keycode: KeyCode::End,
                } => {
                    let max = max_scroll();
                    let mut sy = scroll_y_key.borrow_mut();
                    if (*sy - max).abs() > 0.5 {
                        *sy = max;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            };
            if changed {
                paint_signal_key.notify();
            }
            changed
        })
        .create_element()
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}
