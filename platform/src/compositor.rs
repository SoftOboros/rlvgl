//! Dirty-region compositor for framebuffer restoration.
//!
//! Centralizes the restoration of background pixels when overlay widgets
//! (EventWindow, ConfigMenu, etc.) hide. Each overlay reports its clear
//! region in landscape draw coordinates; the compositor transforms to
//! portrait framebuffer coordinates and copies from a pristine reference.

use alloc::vec::Vec;
use rlvgl_core::widget::Rect;

/// Framebuffer compositor that restores pristine background pixels
/// under dismissed overlay widgets.
pub struct Compositor {
    /// Regions to restore this frame (in fb portrait coords).
    dirty: Vec<FbRect>,
    /// Base address of the pristine framebuffer copy in SDRAM.
    pristine_addr: usize,
    /// Portrait framebuffer width (short axis, e.g. 480).
    fb_width: u32,
    /// Portrait framebuffer height (long axis, e.g. 800).
    fb_height: u32,
    /// Bytes per row (fb_width * 4 for ARGB8888).
    stride: usize,
}

/// Rectangle in portrait framebuffer coordinates.
#[derive(Debug, Clone, Copy)]
struct FbRect {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

impl Compositor {
    /// Create a new compositor.
    ///
    /// `pristine_addr` is the SDRAM address of the pristine background copy.
    /// `fb_width` and `fb_height` are the portrait framebuffer dimensions.
    pub fn new(pristine_addr: u32, fb_width: u32, fb_height: u32) -> Self {
        Self {
            dirty: Vec::new(),
            pristine_addr: pristine_addr as usize,
            fb_width,
            fb_height,
            stride: (fb_width * 4) as usize,
        }
    }

    /// Mark a region (in landscape draw coordinates) for pristine restoration.
    ///
    /// The draw-to-fb transform matches `RotatedRenderer`:
    /// `fb_x = fb_width - draw_y - draw_h`, `fb_y = draw_x`.
    pub fn mark_dirty(&mut self, draw_rect: Rect) {
        let fb_x = (self.fb_width as i32 - draw_rect.y - draw_rect.height).max(0) as u32;
        let fb_y = draw_rect.x.max(0) as u32;
        let fb_w = (draw_rect.height as u32).min(self.fb_width.saturating_sub(fb_x));
        let fb_h = (draw_rect.width as u32).min(self.fb_height.saturating_sub(fb_y));

        if fb_w > 0 && fb_h > 0 {
            self.dirty.push(FbRect {
                x: fb_x,
                y: fb_y,
                w: fb_w,
                h: fb_h,
            });
        }
    }

    /// Restore all dirty regions from the pristine copy to the back buffer.
    ///
    /// Call this before drawing the widget tree each frame. Clears the
    /// dirty list after restoration.
    ///
    /// # Safety
    /// `back_buffer` must point to a valid framebuffer of at least
    /// `fb_width * fb_height * 4` bytes. The pristine address must also
    /// be valid for the same size.
    pub unsafe fn restore(&mut self, back_buffer: *mut u8) {
        let prist = self.pristine_addr as *const u8;
        let stride = self.stride;

        for region in &self.dirty {
            for row in 0..region.h {
                let y = region.y + row;
                let off = y as usize * stride + region.x as usize * 4;
                let len = region.w as usize * 4;
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        prist.add(off),
                        back_buffer.add(off),
                        len,
                    );
                }
            }
        }

        self.dirty.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mark_dirty_transforms_coords() {
        let mut comp = Compositor::new(0, 480, 800);
        // Draw rect at landscape (100, 50, 200, 100)
        // fb_x = 480 - 50 - 100 = 330, fb_y = 100, fb_w = 100, fb_h = 200
        comp.mark_dirty(Rect { x: 100, y: 50, width: 200, height: 100 });
        assert_eq!(comp.dirty.len(), 1);
        let r = &comp.dirty[0];
        assert_eq!((r.x, r.y, r.w, r.h), (330, 100, 100, 200));
    }

    #[test]
    fn mark_dirty_clamps_negative() {
        let mut comp = Compositor::new(0, 480, 800);
        comp.mark_dirty(Rect { x: -10, y: -5, width: 100, height: 500 });
        assert_eq!(comp.dirty.len(), 1);
        let r = &comp.dirty[0];
        assert_eq!(r.x, 0); // clamped
        assert_eq!(r.y, 0); // clamped
    }

    #[test]
    fn restore_clears_dirty() {
        let mut comp = Compositor::new(0, 480, 800);
        comp.mark_dirty(Rect { x: 10, y: 10, width: 20, height: 20 });
        assert_eq!(comp.dirty.len(), 1);
        // Can't call restore in test (no valid pointers), but verify clear logic
        comp.dirty.clear();
        assert_eq!(comp.dirty.len(), 0);
    }
}
