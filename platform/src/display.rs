use alloc::vec;
use alloc::vec::Vec;
use rlvgl_core::widget::{Color, Rect};

/// Trait implemented by display drivers
pub trait DisplayDriver {
    /// Flush a rectangular region of pixels to the display
    fn flush(&mut self, area: Rect, colors: &[Color]);

    /// Optional vertical sync hook
    fn vsync(&mut self) {}
}

/// Dummy headless driver used for tests
pub struct DummyDisplay;

impl DisplayDriver for DummyDisplay {
    fn flush(&mut self, _area: Rect, _colors: &[Color]) {}
}

/// In-memory framebuffer driver for tests and headless rendering
pub struct BufferDisplay {
    pub width: usize,
    pub height: usize,
    pub buffer: Vec<Color>,
}

impl BufferDisplay {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            buffer: vec![Color(0, 0, 0); width * height],
        }
    }
}

impl DisplayDriver for BufferDisplay {
    fn flush(&mut self, area: Rect, colors: &[Color]) {
        for y in 0..area.height as usize {
            for x in 0..area.width as usize {
                let idx = (area.y as usize + y) * self.width + (area.x as usize + x);
                self.buffer[idx] = colors[y * area.width as usize + x];
            }
        }
    }
}
