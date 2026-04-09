//! Save-under compositor for overlay framebuffer management.
//!
//! When an overlay becomes visible, the compositor saves the framebuffer
//! pixels under it. When the overlay hides, the saved pixels are restored.
//! This avoids needing a global pristine copy and handles overlapping
//! overlays correctly (each saves what was under it at open time).

use alloc::vec::Vec;
use rlvgl_core::widget::Rect;

/// Portrait framebuffer rectangle (after draw→fb coordinate transform).
#[derive(Debug, Clone, Copy)]
struct FbRect {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

/// A saved framebuffer region with its pixel data.
struct SaveUnder {
    region: FbRect,
    /// Saved ARGB8888 pixel data (row-major, `w * h * 4` bytes).
    pixels: Vec<u8>,
}

/// Compositor for overlay framebuffer management.
///
/// Supports two restore modes:
/// - **Pristine restore**: copies from a fixed reference framebuffer (fast, no allocation)
/// - **Save-under**: saves pixels on show, restores on hide (handles dynamic content)
///
/// All public methods accept landscape draw coordinates; the compositor
/// transforms to portrait framebuffer coordinates internally.
pub struct Compositor {
    /// Active save-under buffers, keyed by overlay ID.
    saves: Vec<(u32, SaveUnder)>,
    /// Pending restores for this frame (save-under).
    pending_restore: Vec<SaveUnder>,
    /// Pending pristine restores with frame countdown.
    pending_pristine: Vec<(FbRect, u8)>,
    /// Pristine framebuffer address (if set).
    pristine_addr: usize,
    /// Portrait framebuffer width (short axis, e.g. 480).
    fb_width: u32,
    /// Portrait framebuffer height (long axis, e.g. 800).
    fb_height: u32,
    /// Bytes per row.
    stride: usize,
    /// Next overlay ID.
    next_id: u32,
    /// Number of pristine regions copied during the most recent restore pass.
    last_pristine_regions: u16,
    /// Number of save-under regions copied during the most recent restore pass.
    last_save_regions: u16,
    /// Total bytes copied during the most recent restore pass.
    last_restore_bytes: u32,
    /// Monotonic restore sequence number.
    restore_seq: u32,
}

impl Compositor {
    /// Create a new compositor for the given portrait framebuffer dimensions.
    ///
    /// `pristine_addr` is the address of a reference background copy in SDRAM
    /// (e.g. the desktop image). Pass 0 if no pristine copy is available.
    pub fn new(fb_width: u32, fb_height: u32, pristine_addr: u32) -> Self {
        Self {
            saves: Vec::new(),
            pending_restore: Vec::new(),
            pending_pristine: Vec::new(),
            pristine_addr: pristine_addr as usize,
            fb_width,
            fb_height,
            stride: (fb_width * 4) as usize,
            next_id: 1,
            last_pristine_regions: 0,
            last_save_regions: 0,
            last_restore_bytes: 0,
            restore_seq: 0,
        }
    }

    /// Allocate an overlay ID for save-under management.
    pub fn register_overlay(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Transform landscape draw rect to portrait fb rect.
    fn draw_to_fb(&self, draw_rect: Rect) -> FbRect {
        let fb_x = (self.fb_width as i32 - draw_rect.y - draw_rect.height).max(0) as u32;
        let fb_y = draw_rect.x.max(0) as u32;
        let fb_w = (draw_rect.height as u32).min(self.fb_width.saturating_sub(fb_x));
        let fb_h = (draw_rect.width as u32).min(self.fb_height.saturating_sub(fb_y));
        FbRect {
            x: fb_x,
            y: fb_y,
            w: fb_w,
            h: fb_h,
        }
    }

    /// Save the framebuffer pixels under an overlay region.
    ///
    /// Call when an overlay becomes visible. `draw_rect` is in landscape
    /// draw coordinates. Reads from the **front** buffer (what's currently
    /// displayed) since the back buffer may not have been rendered yet.
    ///
    /// # Safety
    /// `front_buffer` must point to a valid framebuffer.
    pub unsafe fn save(&mut self, overlay_id: u32, draw_rect: Rect, front_buffer: *const u8) {
        let fb = self.draw_to_fb(draw_rect);
        if fb.w == 0 || fb.h == 0 {
            return;
        }
        let stride = self.stride;
        let mut pixels = Vec::with_capacity((fb.w * fb.h * 4) as usize);
        for row in 0..fb.h {
            let y = fb.y + row;
            let off = y as usize * stride + fb.x as usize * 4;
            let len = fb.w as usize * 4;
            unsafe {
                let src = core::slice::from_raw_parts(front_buffer.add(off), len);
                pixels.extend_from_slice(src);
            }
        }
        // Remove any existing save for this overlay
        self.saves.retain(|(id, _)| *id != overlay_id);
        self.saves
            .push((overlay_id, SaveUnder { region: fb, pixels }));
    }

    /// Queue restoration from the pristine background copy.
    ///
    /// Use this for overlays over static backgrounds (desktop image).
    /// No heap allocation — reads directly from the pristine fb address.
    /// Queue restoration from pristine for multiple frames (double-buffer).
    pub fn mark_pristine_restore(&mut self, draw_rect: Rect) {
        let fb = self.draw_to_fb(draw_rect);
        if fb.w > 0 && fb.h > 0 {
            // 3 frames: both buffers + 1 safety margin
            self.pending_pristine.push((fb, 3));
        }
    }

    /// Queue restoration of the saved pixels for an overlay.
    ///
    /// Call when an overlay hides. The actual restore happens on the next
    /// `restore()` call. The save buffer is consumed.
    pub fn mark_restore(&mut self, overlay_id: u32) {
        if let Some(pos) = self.saves.iter().position(|(id, _)| *id == overlay_id) {
            let (_, save) = self.saves.remove(pos);
            self.pending_restore.push(save);
        }
    }

    /// Restore all pending save-under regions to the back buffer.
    ///
    /// Call before drawing the widget tree each frame.
    ///
    /// # Safety
    /// `back_buffer` must point to a valid framebuffer.
    pub unsafe fn restore(&mut self, back_buffer: *mut u8) {
        let stride = self.stride;
        let mut pristine_regions = 0u16;
        let mut save_regions = 0u16;
        let mut restore_bytes = 0u32;

        // Restore from pristine background copy (multi-frame for double-buffer)
        if self.pristine_addr != 0 {
            let prist = self.pristine_addr as *const u8;
            for (region, _) in &self.pending_pristine {
                pristine_regions = pristine_regions.saturating_add(1);
                for row in 0..region.h {
                    let y = region.y + row;
                    let off = y as usize * stride + region.x as usize * 4;
                    let len = region.w as usize * 4;
                    unsafe {
                        core::ptr::copy_nonoverlapping(prist.add(off), back_buffer.add(off), len);
                    }
                    restore_bytes = restore_bytes.saturating_add(len as u32);
                }
            }
        }
        // Decrement countdowns, remove expired
        for entry in &mut self.pending_pristine {
            entry.1 = entry.1.saturating_sub(1);
        }
        self.pending_pristine.retain(|e| e.1 > 0);

        // Restore from save-under buffers
        for save in &self.pending_restore {
            save_regions = save_regions.saturating_add(1);
            let fb = &save.region;
            let row_bytes = fb.w as usize * 4;
            for row in 0..fb.h {
                let y = fb.y + row;
                let fb_off = y as usize * stride + fb.x as usize * 4;
                let save_off = row as usize * row_bytes;
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        save.pixels.as_ptr().add(save_off),
                        back_buffer.add(fb_off),
                        row_bytes,
                    );
                }
                restore_bytes = restore_bytes.saturating_add(row_bytes as u32);
            }
        }
        self.pending_restore.clear();
        self.last_pristine_regions = pristine_regions;
        self.last_save_regions = save_regions;
        self.last_restore_bytes = restore_bytes;
        self.restore_seq = self.restore_seq.wrapping_add(1);
    }

    /// Whether there are pending restores of any kind.
    pub fn has_pending(&self) -> bool {
        !self.pending_restore.is_empty() || !self.pending_pristine.is_empty()
    }

    /// Return a packed summary of compositor queue state.
    pub fn diag_counts(&self) -> u32 {
        ((self.saves.len().min(0xFF) as u32) << 24)
            | ((self.pending_restore.len().min(0xFF) as u32) << 16)
            | ((self.pending_pristine.len().min(0xFF) as u32) << 8)
            | ((self.last_pristine_regions.min(0x0F) as u32) << 4)
            | (self.last_save_regions.min(0x0F) as u32)
    }

    /// Return the most recent restore sequence and byte count.
    pub fn diag_bytes(&self) -> u32 {
        ((self.restore_seq & 0xFFFF) << 16) | (self.last_restore_bytes.min(0xFFFF))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_to_fb_transforms_correctly() {
        let comp = Compositor::new(480, 800, 0);
        let fb = comp.draw_to_fb(Rect {
            x: 100,
            y: 50,
            width: 200,
            height: 100,
        });
        // fb_x = 480 - 50 - 100 = 330, fb_y = 100, fb_w = 100, fb_h = 200
        assert_eq!((fb.x, fb.y, fb.w, fb.h), (330, 100, 100, 200));
    }

    #[test]
    fn register_overlay_increments() {
        let mut comp = Compositor::new(480, 800, 0);
        assert_eq!(comp.register_overlay(), 1);
        assert_eq!(comp.register_overlay(), 2);
    }
}
