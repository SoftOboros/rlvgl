//! DMA2D-accelerated overlay drawing with built-in 90° CCW rotation.
//!
//! All public methods accept **landscape** widget coordinates (800×480) and
//! internally rotate to portrait framebuffer coordinates (480×800).
//!
//! This module bypasses the `Renderer` trait pipeline entirely, writing
//! directly to the SDRAM framebuffer via DMA2D hardware operations.

use crate::blit::PixelFmt;
use crate::dma2d::Dma2dBlitter;
use rlvgl_core::packed_font::PackedFont;
use rlvgl_core::widget::{Color, Rect};

/// Minimum pixel count before using DMA2D instead of CPU.
const DMA2D_THRESHOLD: u32 = 64;

/// Context for overlay rendering on a portrait framebuffer.
///
/// All public methods accept **landscape** widget coordinates (800×480) and
/// internally rotate to portrait FB coordinates (480×800).
/// Uses DMA2D hardware for bulk operations and CPU for small ones.
pub struct Dma2dOverlayCtx {
    /// Back-buffer base address in SDRAM.
    pub fb: *mut u8,
    /// Bytes per framebuffer row (fb_w × 4 for ARGB8888).
    pub fb_stride: u32,
    /// Portrait framebuffer width (480 on STM32H747I-DISCO).
    pub fb_w: u32,
    /// Portrait framebuffer height (800 on STM32H747I-DISCO).
    pub fb_h: u32,
    /// When `true`, EoR gate confirmed safe write window — skip per-row delay.
    pub eor_gated: bool,
    /// DWT cycles waited for EoR (diagnostic). u32::MAX = timeout.
    pub eor_wait_cycles: u32,
    /// Optional DMA2D blitter for hardware-accelerated fills and blends.
    /// Raw pointer to avoid lifetime propagation; caller must ensure validity.
    pub dma2d: Option<*mut Dma2dBlitter>,
}

impl Dma2dOverlayCtx {
    // ── Coordinate rotation ────────────────────────────────────────────

    /// Rotate a landscape widget rect to portrait FB coordinates.
    /// Returns `(fb_x, fb_y, fb_w, fb_h)` or `None` if fully off-screen.
    fn rotate_clip(
        &self,
        wx: i32,
        wy: i32,
        ww: i32,
        wh: i32,
    ) -> Option<(i32, i32, i32, i32)> {
        let mut fx = self.fb_w as i32 - wy - wh;
        let fy = wx;
        let mut fw = wh;
        let fh = ww;

        if fw <= 0 || fh <= 0 {
            return None;
        }

        // Clip left
        if fx < 0 {
            fw += fx;
            fx = 0;
        }
        // Clip right
        if fx + fw > self.fb_w as i32 {
            fw = self.fb_w as i32 - fx;
        }
        if fw <= 0 {
            return None;
        }
        // Clip top/bottom
        if fy < 0 || fy + fh > self.fb_h as i32 {
            return None;
        }

        Some((fx, fy, fw, fh))
    }

    /// Pointer to a pixel at portrait FB coordinates.
    #[inline]
    fn fb_ptr(&self, fx: i32, fy: i32) -> *mut u8 {
        unsafe {
            self.fb
                .add(fy as usize * self.fb_stride as usize + fx as usize * 4)
        }
    }

    // ── Rectangle fills ────────────────────────────────────────────────

    /// Fill a landscape-coordinate rectangle with solid color.
    /// Uses DMA2D R2M fill for large areas, CPU writes for small ones.
    pub fn fill_rect_rotated(&mut self, wx: i32, wy: i32, ww: i32, wh: i32, color: Color) {
        if let Some((fx, fy, fw, fh)) = self.rotate_clip(wx, wy, ww, wh) {
            let argb = color.to_argb8888();

            // Use DMA2D for large fills
            if fw as u32 * fh as u32 >= DMA2D_THRESHOLD {
                if let Some(dma2d_ptr) = self.dma2d {
                    let dst = self.fb_ptr(fx, fy);
                    unsafe {
                        (*dma2d_ptr).fill_raw(
                            dst,
                            self.fb_stride,
                            fw as u32,
                            fh as u32,
                            argb,
                            PixelFmt::Argb8888,
                        );
                    }
                    return;
                }
            }

            // CPU fallback for small fills or when DMA2D unavailable
            for row in 0..fh {
                let ptr = self.fb_ptr(fx, fy + row) as *mut u32;
                for col in 0..fw {
                    unsafe { ptr.add(col as usize).write_volatile(argb) };
                }
                if !self.eor_gated {
                    cortex_m::asm::delay(1000);
                }
            }
        }
    }

    /// CPU source-over blend of a single pixel at portrait FB coordinates.
    /// Faster than DMA2D for 1×1 operations (avoids register setup overhead).
    fn blend_pixel_cpu(&self, fx: i32, fy: i32, color: Color) {
        if fx < 0 || fx >= self.fb_w as i32 || fy < 0 || fy >= self.fb_h as i32 {
            return;
        }
        let alpha = color.3 as u16;
        if alpha == 0 {
            return;
        }
        let ptr = self.fb_ptr(fx, fy);
        unsafe {
            if alpha == 255 {
                let argb = color.to_argb8888();
                (ptr as *mut u32).write_volatile(argb);
            } else {
                let inv = 255 - alpha;
                let bg_b = *ptr.add(0) as u16;
                let bg_g = *ptr.add(1) as u16;
                let bg_r = *ptr.add(2) as u16;
                *ptr.add(0) = ((color.2 as u16 * alpha + bg_b * inv) / 255) as u8;
                *ptr.add(1) = ((color.1 as u16 * alpha + bg_g * inv) / 255) as u8;
                *ptr.add(2) = ((color.0 as u16 * alpha + bg_r * inv) / 255) as u8;
                *ptr.add(3) = 0xFF;
            }
        }
    }

    /// Blend a single pixel at landscape widget coordinates.
    fn blend_pixel_rotated(&self, wx: i32, wy: i32, color: Color) {
        let fx = self.fb_w as i32 - 1 - wy;
        let fy = wx;
        self.blend_pixel_cpu(fx, fy, color);
    }

    // ── Rounded rectangles ─────────────────────────────────────────────

    /// Fill a rounded rectangle via CPU fills + AA fringe blends.
    pub fn fill_rounded_rect_hw(&mut self, rect: Rect, color: Color, radius: u8) {
        let r = (radius as i32).min(rect.width / 2).min(rect.height / 2);
        if r <= 0 {
            self.fill_rect_rotated(rect.x, rect.y, rect.width, rect.height, color);
            return;
        }

        // Body: full-width strip between top-radius and bottom-radius
        if rect.height - 2 * r > 0 {
            self.fill_rect_rotated(
                rect.x,
                rect.y + r,
                rect.width,
                rect.height - 2 * r,
                color,
            );
        }

        // Top strip between corners
        if rect.width - 2 * r > 0 {
            self.fill_rect_rotated(
                rect.x + r,
                rect.y,
                rect.width - 2 * r,
                r,
                color,
            );
            // Bottom strip
            self.fill_rect_rotated(
                rect.x + r,
                rect.y + rect.height - r,
                rect.width - 2 * r,
                r,
                color,
            );
        }

        // Corner arcs: per-pixel fill with correct distance formula
        // for each corner. Arc center is at (r, r) offset from each corner.
        let r2 = (r * r) as u32;
        for cy in 0..r {
            for cx in 0..r {
                // Top-left: arc center at (+r, +r), distance = (r-cx, r-cy)
                let d2_tl = ((r - cx) * (r - cx) + (r - cy) * (r - cy)) as u32;
                if d2_tl <= r2 {
                    self.blend_pixel_rotated(rect.x + cx, rect.y + cy, color);
                }
                // Top-right: arc center at (width-r, +r), distance = (cx, r-cy)
                let d2_tr = (cx * cx + (r - cy) * (r - cy)) as u32;
                if d2_tr <= r2 {
                    self.blend_pixel_rotated(rect.x + rect.width - r + cx, rect.y + cy, color);
                }
                // Bottom-left: arc center at (+r, height-r), distance = (r-cx, cy)
                let d2_bl = ((r - cx) * (r - cx) + cy * cy) as u32;
                if d2_bl <= r2 {
                    self.blend_pixel_rotated(rect.x + cx, rect.y + rect.height - r + cy, color);
                }
                // Bottom-right: arc center at (width-r, height-r), distance = (cx, cy)
                let d2_br = (cx * cx + cy * cy) as u32;
                if d2_br <= r2 {
                    self.blend_pixel_rotated(rect.x + rect.width - r + cx, rect.y + rect.height - r + cy, color);
                }
            }
        }
    }

    // ── Borders ────────────────────────────────────────────────────────

    /// Draw a straight (non-rounded) border via 4 DMA2D fills.
    pub fn draw_border_hw(&mut self, rect: Rect, color: Color, width: u8) {
        let w = width as i32;
        if w == 0 {
            return;
        }
        // Top
        self.fill_rect_rotated(rect.x, rect.y, rect.width, w, color);
        // Bottom
        self.fill_rect_rotated(rect.x, rect.y + rect.height - w, rect.width, w, color);
        // Left
        self.fill_rect_rotated(rect.x, rect.y + w, w, rect.height - 2 * w, color);
        // Right
        self.fill_rect_rotated(
            rect.x + rect.width - w,
            rect.y + w,
            w,
            rect.height - 2 * w,
            color,
        );
    }

    /// Draw a rounded border via CPU fills + AA fringe.
    pub fn draw_rounded_border_hw(
        &mut self,
        rect: Rect,
        color: Color,
        border_width: u8,
        radius: u8,
    ) {
        let bw = border_width as i32;
        if bw == 0 {
            return;
        }
        let rout = (radius as i32).min(rect.width / 2).min(rect.height / 2);
        if rout <= 0 {
            self.draw_border_hw(rect, color, border_width);
            return;
        }
        let rin = (rout - bw).max(0);
        let base_alpha = color.3 as u16;

        // Corner arcs (ring between outer and inner radius)
        for dy in 0..rout {
            let (out_dx, out_frac) = arc_dx(rout, dy);
            let (in_dx, in_frac) = if rin > 0 { arc_dx(rin, dy) } else { (0, 0) };

            let ring_w = out_dx - in_dx;
            if ring_w > 0 {
                self.fill_rect_rotated(
                    rect.x + rout - out_dx, rect.y + dy, ring_w, 1, color,
                );
                self.fill_rect_rotated(
                    rect.x + rect.width - rout + in_dx, rect.y + dy, ring_w, 1, color,
                );
                self.fill_rect_rotated(
                    rect.x + rout - out_dx, rect.y + rect.height - 1 - dy, ring_w, 1, color,
                );
                self.fill_rect_rotated(
                    rect.x + rect.width - rout + in_dx,
                    rect.y + rect.height - 1 - dy,
                    ring_w,
                    1,
                    color,
                );
            }

            // Outer AA fringe
            if out_frac > 0 {
                let aa = Color(
                    color.0, color.1, color.2,
                    ((out_frac as u16 * base_alpha) / 255) as u8,
                );
                self.blend_pixel_rotated(rect.x + rout - out_dx - 1, rect.y + dy, aa);
                self.blend_pixel_rotated(
                    rect.x + rect.width - rout + out_dx, rect.y + dy, aa,
                );
                self.blend_pixel_rotated(
                    rect.x + rout - out_dx - 1, rect.y + rect.height - 1 - dy, aa,
                );
                self.blend_pixel_rotated(
                    rect.x + rect.width - rout + out_dx,
                    rect.y + rect.height - 1 - dy,
                    aa,
                );
            }

            // Inner AA fringe
            if in_dx > 0 && in_frac > 0 {
                let aa = Color(
                    color.0, color.1, color.2,
                    (((255 - in_frac as u16) * base_alpha) / 255) as u8,
                );
                self.blend_pixel_rotated(rect.x + rout - in_dx, rect.y + dy, aa);
                self.blend_pixel_rotated(
                    rect.x + rect.width - rout + in_dx - 1, rect.y + dy, aa,
                );
                self.blend_pixel_rotated(
                    rect.x + rout - in_dx, rect.y + rect.height - 1 - dy, aa,
                );
                self.blend_pixel_rotated(
                    rect.x + rect.width - rout + in_dx - 1,
                    rect.y + rect.height - 1 - dy,
                    aa,
                );
            }
        }

        // Straight border segments between corners
        let straight_h = rect.height - 2 * rout;
        if straight_h > 0 {
            self.fill_rect_rotated(rect.x, rect.y + rout, bw, straight_h, color);
            self.fill_rect_rotated(
                rect.x + rect.width - bw, rect.y + rout, bw, straight_h, color,
            );
        }
        let straight_w = rect.width - 2 * rout;
        if straight_w > 0 {
            self.fill_rect_rotated(rect.x + rout, rect.y, straight_w, bw, color);
            self.fill_rect_rotated(
                rect.x + rout, rect.y + rect.height - bw, straight_w, bw, color,
            );
        }
    }

    // ── Text rendering ─────────────────────────────────────────────────

    /// Render a single glyph at landscape (gx, gy) using DMA2D A8 blend.
    ///
    /// `glyph_data` is the raw A8 alpha bitmap (row-major, `gw × gh`).
    /// `scratch` is a caller-provided buffer in DMA2D-accessible memory
    /// (SDRAM) for the rotated A8 data. Must be ≥ `gw * gh` bytes.
    pub fn draw_glyph_rotated(
        &mut self,
        gx: i32,
        gy: i32,
        glyph_data: &[u8],
        gw: u32,
        gh: u32,
        fg_color: Color,
        scratch: &mut [u8],
    ) {
        let total = (gw * gh) as usize;
        if total == 0 || total > scratch.len() || glyph_data.len() < total {
            return;
        }

        // Rotated FB destination
        let fx = self.fb_w as i32 - gy as i32 - gh as i32;
        let fy = gx;

        // Clip check
        if fx < 0 || fy < 0 || fx + gh as i32 > self.fb_w as i32 || fy + gw as i32 > self.fb_h as i32
        {
            return;
        }

        // Rotate A8 bitmap 90° CCW into scratch buffer.
        // Original pixel at (col, row) → rotated at (col * gh + (gh - 1 - row)).
        // Rotated dimensions: gh wide × gw tall.
        for row in 0..gh as usize {
            for col in 0..gw as usize {
                let src_alpha = glyph_data[row * gw as usize + col];
                scratch[col * gh as usize + (gh as usize - 1 - row)] = src_alpha;
            }
        }

        // Rotated glyph dimensions: gh wide × gw tall
        let rot_w = gh;
        let rot_h = gw;

        // Use DMA2D A8 blend for large glyphs
        if rot_w * rot_h >= DMA2D_THRESHOLD {
            if let Some(dma2d_ptr) = self.dma2d {
                // Pre-modulate scratch alpha by fg_color.3 if not fully opaque
                if fg_color.3 != 255 {
                    let base = fg_color.3 as u16;
                    for i in 0..(rot_w * rot_h) as usize {
                        scratch[i] = ((scratch[i] as u16 * base) / 255) as u8;
                    }
                }
                let fg_rgb = ((fg_color.0 as u32) << 16)
                    | ((fg_color.1 as u32) << 8)
                    | (fg_color.2 as u32);
                let dst = self.fb_ptr(fx, fy);
                unsafe {
                    (*dma2d_ptr).blend_a8_color(
                        scratch.as_ptr(),
                        rot_w,
                        rot_h,
                        fg_rgb,
                        dst,
                        self.fb_stride,
                    );
                }
                return;
            }
        }

        // CPU source-over blend fallback
        let alpha_base = fg_color.3 as u16;
        for rot_row in 0..gw as usize {
            for rot_col in 0..gh as usize {
                let a8 = scratch[rot_row * gh as usize + rot_col] as u16;
                if a8 == 0 {
                    continue;
                }
                let alpha = (a8 * alpha_base) / 255;
                let px = fx + rot_col as i32;
                let py = fy + rot_row as i32;
                if px < 0 || px >= self.fb_w as i32 || py < 0 || py >= self.fb_h as i32 {
                    continue;
                }
                let ptr = self.fb_ptr(px, py);
                unsafe {
                    if alpha >= 255 {
                        let argb = fg_color.to_argb8888();
                        (ptr as *mut u32).write_volatile(argb);
                    } else {
                        let inv = 255 - alpha;
                        let bg_b = *ptr.add(0) as u16;
                        let bg_g = *ptr.add(1) as u16;
                        let bg_r = *ptr.add(2) as u16;
                        *ptr.add(0) = ((fg_color.2 as u16 * alpha + bg_b * inv) / 255) as u8;
                        *ptr.add(1) = ((fg_color.1 as u16 * alpha + bg_g * inv) / 255) as u8;
                        *ptr.add(2) = ((fg_color.0 as u16 * alpha + bg_r * inv) / 255) as u8;
                        *ptr.add(3) = 0xFF;
                    }
                }
            }
        }
    }

    /// Render a string at landscape (x, y) using DMA2D glyph blending.
    ///
    /// `scratch` must be in DMA2D-accessible memory (SDRAM), ≥ 512 bytes.
    pub fn draw_str_rotated(
        &mut self,
        font: &PackedFont,
        x: i32,
        y: i32,
        text: &str,
        color: Color,
        scratch: &mut [u8],
    ) {
        let mut cx = x;
        for ch in text.chars() {
            if let Some(glyph) = font.glyph(ch) {
                let gw = glyph.width as u32;
                let gh = glyph.height as u32;
                let off = glyph.offset as usize;
                let total = (gw * gh) as usize;

                // Position glyph relative to baseline using ymin
                let gy = y + font.ascent as i32 - glyph.ymin as i32 - gh as i32;

                if total > 0 && total <= scratch.len() {
                    if let Some(data) = font.data.get(off..off + total) {
                        self.draw_glyph_rotated(cx, gy, data, gw, gh, color, scratch);
                    }
                }
                cx += (glyph.advance_fp16 as i32 + 8) >> 4;
            } else {
                cx += font.height as i32 / 2;
            }
        }
    }
}

// ── Arc math (copied from core/src/draw.rs) ────────────────────────────

/// Integer square root (floor).
fn isqrt(n: u32) -> u32 {
    if n == 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// Arc x-extent at row `dy` for radius `r`. Returns (integer_dx, 0-255 frac).
fn arc_dx(r: i32, dy: i32) -> (i32, u8) {
    let r4 = r as u32 * 4;
    let dy4 = dy as u32 * 4 + 2;
    let sq = r4 * r4;
    let dysq = dy4 * dy4;
    if dysq >= sq {
        return (0, 0);
    }
    let dx4 = isqrt(sq - dysq);
    let dx_int = (dx4 / 4) as i32;
    let frac = (dx4 % 4) as u8 * 64;
    (dx_int, frac)
}
