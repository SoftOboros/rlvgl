// SPDX-License-Identifier: MIT
//! Jumbo background buffer shared by scrolling motion widgets.
//!
//! A "jumbo" buffer is a view over an externally-supplied slice that
//! is **bigger than the visible window along the scroll axis**, so the
//! scroll cursor can slide across it cheaply while the background
//! pattern only needs to be painted once. When the cursor reaches the
//! end of the head half the caller can use [`JumboBuffer::mirror_tail`]
//! to copy the head into the tail so the next scroll cycle wraps
//! seamlessly, or the window math in [`JumboBuffer::window_range`]
//! can wrap the read window around the buffer edge.
//!
//! [`JumboBuffer`] does **not** own its memory. The caller supplies a
//! `&mut [u8]` slice sized for the full jumbo footprint, which lets
//! embedded targets place the buffer in SDRAM at a fixed address while
//! host targets use `Vec<u8>`. The exact same pattern
//! [`rlvgl_platform::blit::Surface`] already uses for its pixel
//! storage.
//!
//! Any existing [`rlvgl_platform::blit::Blitter`] implementation can
//! operate on a jumbo buffer by calling
//! [`JumboBuffer::as_surface`] — the jumbo is a `Surface` when asked,
//! so the blit path composes with normal screen blits without a
//! parallel code path.

use rlvgl_platform::blit::{PixelFmt, Surface};

/// Orientation of the jumbo buffer's long axis.
///
/// Vertical jumbos are taller than the visible window (used by
/// up/down scrolls); horizontal jumbos are wider (used by left/right
/// scrolls).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum JumboOrientation {
    /// Buffer is wider than the visible window; scroll axis is X.
    Horizontal,
    /// Buffer is taller than the visible window; scroll axis is Y.
    Vertical,
}

/// A view over an externally-supplied scrollable background buffer.
///
/// The buffer is laid out as an N× multiple of the visible window in
/// the orientation's long axis. Callers scatter a background pattern
/// into the head half, optionally [`mirror_tail`](Self::mirror_tail)
/// to duplicate it into the tail, then use
/// [`window_range`](Self::window_range) to compute the pixel slice
/// that should be copied to the screen for a given scroll offset.
pub struct JumboBuffer<'a> {
    /// Raw pixel storage. Caller owns the lifetime.
    pub buf: &'a mut [u8],
    /// Bytes per row of the buffer.
    pub stride: usize,
    /// Visible window width in pixels (not bytes).
    pub visible_w: u32,
    /// Visible window height in pixels.
    pub visible_h: u32,
    /// Long-axis multiplier. `2` for a double-sized jumbo, `3` for
    /// triple, etc. Must be at least `1`.
    pub scale: u8,
    /// Long axis orientation.
    pub orientation: JumboOrientation,
    /// Underlying pixel format of the buffer. Used by `as_surface`
    /// when producing a `Surface` view.
    pub format: PixelFmt,
}

impl<'a> JumboBuffer<'a> {
    /// Create a new jumbo buffer view.
    ///
    /// The caller must supply a slice sized for
    /// `stride * (visible_h * scale)` bytes for vertical orientation,
    /// or `stride * visible_h` bytes (where `stride == visible_w * scale * bpp`)
    /// for horizontal orientation. The constructor does not validate
    /// the slice length — a too-small slice will simply fail later
    /// inside [`as_surface`] or row index math.
    #[inline]
    pub fn new(
        buf: &'a mut [u8],
        stride: usize,
        visible_w: u32,
        visible_h: u32,
        scale: u8,
        orientation: JumboOrientation,
        format: PixelFmt,
    ) -> Self {
        Self {
            buf,
            stride,
            visible_w,
            visible_h,
            scale,
            orientation,
            format,
        }
    }

    /// Full long-axis extent in pixels.
    #[inline]
    pub const fn long_axis_pixels(&self) -> u32 {
        let base = match self.orientation {
            JumboOrientation::Horizontal => self.visible_w,
            JumboOrientation::Vertical => self.visible_h,
        };
        base.saturating_mul(self.scale as u32)
    }

    /// Short-axis extent in pixels (unchanged from visible).
    #[inline]
    pub const fn short_axis_pixels(&self) -> u32 {
        match self.orientation {
            JumboOrientation::Horizontal => self.visible_h,
            JumboOrientation::Vertical => self.visible_w,
        }
    }

    /// Expose the full jumbo buffer as a
    /// [`rlvgl_platform::blit::Surface`] that any
    /// [`rlvgl_platform::blit::Blitter`] can blit to or from.
    ///
    /// This is the composition promise: a jumbo buffer **is** a
    /// surface when asked, so the blit path over jumbo buffers is the
    /// same blit path that operates on the screen back buffer.
    pub fn as_surface(&mut self) -> Surface<'_> {
        let (w, h) = match self.orientation {
            JumboOrientation::Horizontal => {
                (self.visible_w * self.scale as u32, self.visible_h)
            }
            JumboOrientation::Vertical => {
                (self.visible_w, self.visible_h * self.scale as u32)
            }
        };
        Surface::new(self.buf, self.stride, self.format, w, h)
    }

    /// Compute `(start_pixel, len_pixels)` slabs that together cover a
    /// window of `len` pixels along the scroll axis starting at
    /// `start`, folded around the buffer's long axis.
    ///
    /// Returns two slabs because a window that straddles the buffer
    /// boundary needs a tail slab and a head slab. When the window
    /// fits entirely in one half the second slab has `len == 0`.
    pub fn window_range(&self, start: u32, len: u32) -> ((u32, u32), (u32, u32)) {
        let total = self.long_axis_pixels();
        if total == 0 {
            return ((0, 0), (0, 0));
        }
        let start = start % total;
        let end = start.saturating_add(len);
        if end <= total {
            ((start, len), (0, 0))
        } else {
            let first_len = total - start;
            let second_len = len - first_len;
            ((start, first_len), (0, second_len))
        }
    }

    /// Copy the first visible slab into the tail so a scroll cursor
    /// that slides past the end sees a seamless wrap.
    ///
    /// Only meaningful when `scale >= 2`. No-op otherwise. Assumes the
    /// caller has already populated the head half with a background
    /// pattern; calling this function before that produces a tail of
    /// zeros which is still correct, just not useful.
    pub fn mirror_tail(&mut self) {
        if self.scale < 2 {
            return;
        }
        let base = match self.orientation {
            JumboOrientation::Horizontal => self.visible_w as usize,
            JumboOrientation::Vertical => self.visible_h as usize,
        };
        if base == 0 {
            return;
        }
        match self.orientation {
            JumboOrientation::Vertical => {
                let head_bytes = base * self.stride;
                let total = base * self.scale as usize * self.stride;
                if head_bytes == 0 || total <= head_bytes {
                    return;
                }
                // Fill tail by repeating the head pattern until we fill
                // the buffer. `copy_within` is safe because head and
                // tail don't overlap.
                let mut written = head_bytes;
                while written < total {
                    let take = (total - written).min(head_bytes);
                    self.buf.copy_within(0..take, written);
                    written += take;
                }
            }
            JumboOrientation::Horizontal => {
                // Horizontal scale: each row's tail mirrors that row's
                // head. Process one row at a time.
                let bpp = match self.format {
                    PixelFmt::Argb8888 => 4,
                    PixelFmt::Rgb565 => 2,
                    PixelFmt::L8 | PixelFmt::A8 => 1,
                    PixelFmt::A4 => 1, // 2 pixels per byte, but still 1 byte/step
                };
                let head_bytes_per_row = base * bpp;
                let row_count = self.visible_h as usize;
                for y in 0..row_count {
                    let row_start = y * self.stride;
                    let row_end = row_start + self.stride;
                    if row_end > self.buf.len() {
                        break;
                    }
                    let row = &mut self.buf[row_start..row_end];
                    let mut written = head_bytes_per_row;
                    while written < row.len() {
                        let take = (row.len() - written).min(head_bytes_per_row);
                        row.copy_within(0..take, written);
                        written += take;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn long_axis_vertical_doubles_height() {
        let mut buf = vec![0u8; 16 * 8 * 4];
        let j = JumboBuffer::new(
            &mut buf,
            16 * 4,
            16,
            4,
            2,
            JumboOrientation::Vertical,
            PixelFmt::Argb8888,
        );
        assert_eq!(j.long_axis_pixels(), 8);
        assert_eq!(j.short_axis_pixels(), 16);
    }

    #[test]
    fn long_axis_horizontal_doubles_width() {
        let mut buf = vec![0u8; 32 * 4 * 4];
        let j = JumboBuffer::new(
            &mut buf,
            32 * 4,
            16,
            4,
            2,
            JumboOrientation::Horizontal,
            PixelFmt::Argb8888,
        );
        assert_eq!(j.long_axis_pixels(), 32);
        assert_eq!(j.short_axis_pixels(), 4);
    }

    #[test]
    fn window_range_does_not_wrap() {
        let mut buf = vec![0u8; 4 * 8 * 4];
        let j = JumboBuffer::new(
            &mut buf,
            4 * 4,
            4,
            4,
            2,
            JumboOrientation::Vertical,
            PixelFmt::Argb8888,
        );
        let (a, b) = j.window_range(0, 4);
        assert_eq!(a, (0, 4));
        assert_eq!(b, (0, 0));
    }

    #[test]
    fn window_range_wraps_at_end() {
        let mut buf = vec![0u8; 4 * 8 * 4];
        let j = JumboBuffer::new(
            &mut buf,
            4 * 4,
            4,
            4,
            2,
            JumboOrientation::Vertical,
            PixelFmt::Argb8888,
        );
        // Long axis = 8, window of 4 starting at 6 → 2 slab at [6..8)
        // and 2 slab at [0..2).
        let (a, b) = j.window_range(6, 4);
        assert_eq!(a, (6, 2));
        assert_eq!(b, (0, 2));
    }

    #[test]
    fn window_range_modulo_start() {
        let mut buf = vec![0u8; 4 * 4 * 4];
        let j = JumboBuffer::new(
            &mut buf,
            4 * 4,
            4,
            4,
            2,
            JumboOrientation::Vertical,
            PixelFmt::Argb8888,
        );
        // start = 8 → equivalent to start = 0 (total long axis = 8).
        let (a, b) = j.window_range(8, 4);
        assert_eq!(a, (0, 4));
        assert_eq!(b, (0, 0));
    }

    #[test]
    fn as_surface_reports_full_jumbo_extent() {
        let mut buf = vec![0u8; 4 * 8 * 4];
        let mut j = JumboBuffer::new(
            &mut buf,
            4 * 4,
            4,
            4,
            2,
            JumboOrientation::Vertical,
            PixelFmt::Argb8888,
        );
        let s = j.as_surface();
        assert_eq!(s.width, 4);
        assert_eq!(s.height, 8);
        assert_eq!(s.stride, 16);
    }

    #[test]
    fn mirror_tail_vertical_copies_head_to_tail() {
        // 2 cols × 2 head rows × 2 scale = 4 rows total, 2 cols, ARGB8888.
        let stride = 2 * 4;
        let mut buf = vec![0u8; stride * 4];
        // Paint head row 0 = 0xAA, row 1 = 0xBB. Leave tail zero.
        for b in &mut buf[0..stride] {
            *b = 0xAA;
        }
        for b in &mut buf[stride..stride * 2] {
            *b = 0xBB;
        }
        let mut j = JumboBuffer::new(
            &mut buf,
            stride,
            2,
            2,
            2,
            JumboOrientation::Vertical,
            PixelFmt::Argb8888,
        );
        j.mirror_tail();
        // Tail row 0 should match head row 0, tail row 1 should match head row 1.
        assert!(buf[stride * 2..stride * 3].iter().all(|&b| b == 0xAA));
        assert!(buf[stride * 3..stride * 4].iter().all(|&b| b == 0xBB));
    }

    #[test]
    fn mirror_tail_horizontal_copies_head_to_tail_per_row() {
        // 4 visible cols × 2 rows, scale 2 → 8 cols total, stride = 8*4.
        let stride = 8 * 4;
        let visible_w = 4usize;
        let mut buf = vec![0u8; stride * 2];
        // Row 0 head = 0xCC, row 1 head = 0xDD. Tail zero.
        for y in 0..2 {
            let row_start = y * stride;
            for x in 0..visible_w {
                let off = row_start + x * 4;
                for k in 0..4 {
                    buf[off + k] = if y == 0 { 0xCC } else { 0xDD };
                }
            }
        }
        let mut j = JumboBuffer::new(
            &mut buf,
            stride,
            4,
            2,
            2,
            JumboOrientation::Horizontal,
            PixelFmt::Argb8888,
        );
        j.mirror_tail();
        // Row 0 tail [4..8) cols should be 0xCC, row 1 tail should be 0xDD.
        for y in 0..2 {
            let row_start = y * stride;
            let expected = if y == 0 { 0xCC } else { 0xDD };
            for x in visible_w..(visible_w * 2) {
                let off = row_start + x * 4;
                for k in 0..4 {
                    assert_eq!(buf[off + k], expected, "row {y} col {x} byte {k}");
                }
            }
        }
    }

    #[test]
    fn mirror_tail_scale_one_is_no_op() {
        let stride = 4 * 4;
        let mut buf = vec![0xAA; stride * 4];
        let mut j = JumboBuffer::new(
            &mut buf,
            stride,
            4,
            4,
            1,
            JumboOrientation::Vertical,
            PixelFmt::Argb8888,
        );
        j.mirror_tail();
        // Unchanged.
        assert!(buf.iter().all(|&b| b == 0xAA));
    }
}
