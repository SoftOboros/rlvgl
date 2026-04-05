//! 1-bit bitmap icons for the file browser.
//!
//! Each icon is stored as 1-bit-per-pixel row-major data with MSB mapping to
//! the leftmost pixel. Icons are 8 pixels wide (1 byte per row) for trivial
//! packing. Rendering scales to a `target_height` so the icon matches the
//! active font height.

use crate::renderer::Renderer;
use crate::widget::Color;

/// A 1-bit icon bitmap that can be drawn at any integer scale.
pub struct IconBitmap {
    /// Width of the icon in native pixels.
    pub width: u8,
    /// Height of the icon in native pixels.
    pub height: u8,
    /// 1-bit-per-pixel row-major packed data, MSB = leftmost pixel.
    pub data: &'static [u8],
}

impl IconBitmap {
    /// Draw the icon at `(x, y)` scaled so it reaches `target_height` display
    /// pixels. Each native pixel becomes a `scale x scale` filled rectangle.
    pub fn draw(
        &self,
        renderer: &mut dyn Renderer,
        x: i32,
        y: i32,
        target_height: i32,
        color: Color,
    ) {
        let scale = self.compute_scale(target_height);
        let w = self.width as usize;
        for row in 0..self.height as usize {
            for col in 0..w {
                let bit_index = row * w + col;
                let byte_index = bit_index / 8;
                let bit_offset = 7 - (bit_index % 8);
                if byte_index < self.data.len()
                    && (self.data[byte_index] >> bit_offset) & 1 == 1
                {
                    renderer.fill_rect(
                        crate::widget::Rect {
                            x: x + col as i32 * scale,
                            y: y + row as i32 * scale,
                            width: scale,
                            height: scale,
                        },
                        color,
                    );
                }
            }
        }
    }

    /// Scaled width in display pixels for a given target height.
    pub fn scaled_width(&self, target_height: i32) -> i32 {
        self.width as i32 * self.compute_scale(target_height)
    }

    fn compute_scale(&self, target_height: i32) -> i32 {
        let s = if self.height > 0 {
            target_height / self.height as i32
        } else {
            1
        };
        if s < 1 { 1 } else { s }
    }
}

/// Folder icon (8x10 native pixels, 10 bytes).
///
/// ```text
/// .###....   tab
/// ########   top edge
/// #......#   walls
/// #......#
/// #......#
/// #......#
/// #......#
/// #......#
/// #......#
/// ########   bottom edge
/// ```
pub static ICON_FOLDER: IconBitmap = IconBitmap {
    width: 8,
    height: 10,
    data: &[
        0b0111_0000, // .###....
        0b1111_1111, // ########
        0b1000_0001, // #......#
        0b1000_0001,
        0b1000_0001,
        0b1000_0001,
        0b1000_0001,
        0b1000_0001,
        0b1000_0001,
        0b1111_1111, // ########
    ],
};

/// File icon (8x10 native pixels, 10 bytes).
///
/// ```text
/// ######..   top with dog-ear space
/// #....##.   dog-ear fold
/// #.....#.   body
/// #.....#.
/// #.....#.
/// #.....#.
/// #.....#.
/// #.....#.
/// #.....#.
/// #######.   bottom
/// ```
pub static ICON_FILE: IconBitmap = IconBitmap {
    width: 8,
    height: 10,
    data: &[
        0b1111_1100, // ######..
        0b1000_0110, // #....##.
        0b1000_0010, // #.....#.
        0b1000_0010,
        0b1000_0010,
        0b1000_0010,
        0b1000_0010,
        0b1000_0010,
        0b1000_0010,
        0b1111_1110, // #######.
    ],
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::Rect;

    struct CountRenderer {
        rects: usize,
    }

    impl Renderer for CountRenderer {
        fn fill_rect(&mut self, _rect: Rect, _color: Color) {
            self.rects += 1;
        }
        fn draw_text(&mut self, _pos: (i32, i32), _text: &str, _color: Color) {}
    }

    #[test]
    fn folder_icon_draws_pixels() {
        let mut r = CountRenderer { rects: 0 };
        ICON_FOLDER.draw(&mut r, 0, 0, 20, Color(255, 255, 255, 255));
        assert!(r.rects > 0);
    }

    #[test]
    fn scaled_width_matches() {
        assert_eq!(ICON_FOLDER.scaled_width(20), 16); // 8 * (20/10) = 16
        assert_eq!(ICON_FILE.scaled_width(20), 16);
    }

    #[test]
    fn scale_floor_one() {
        assert_eq!(ICON_FOLDER.scaled_width(5), 8); // 8 * 1
    }
}
