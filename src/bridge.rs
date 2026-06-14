//! RGBA8 (tiny-skia Pixmap) → BGRA (ScarletUI Canvas) pixel bridge.

use tiny_skia::Pixmap;

/// Convert a tiny-skia RGBA8 pixmap into the BGRA byte buffer expected by
/// ScarletUI's `CanvasRenderCallback`.
///
/// ScarletUI Canvas uses BGRA byte order (B, G, R, A in memory).
/// tiny-skia `Pixmap` stores RGBA (R, G, B, A in memory).
/// We swizzle R and B channels in-place.
pub fn pixmap_to_canvas_bgra(pixmap: &Pixmap, buffer: &mut [u8], width: u32, height: u32) {
    let src = pixmap.data();
    let expected_len = (width as usize) * (height as usize) * 4;

    if src.len() != expected_len || buffer.len() != expected_len {
        for b in buffer.iter_mut() {
            *b = 0;
        }
        return;
    }

    // Swizzle RGBA → BGRA (swap R and B bytes in each 4-byte pixel).
    for (src_pixel, dst_pixel) in src.chunks_exact(4).zip(buffer.chunks_exact_mut(4)) {
        dst_pixel[0] = src_pixel[2]; // B ← R(Blue)
        dst_pixel[1] = src_pixel[1]; // G ← G
        dst_pixel[2] = src_pixel[0]; // R ← B(Red)
        dst_pixel[3] = src_pixel[3]; // A ← A
    }
}
