//! Traits and helpers for display drivers.
use alloc::vec;
use alloc::vec::Vec;
use rlvgl_core::widget::{Color, Rect};

use crate::screen::Screen;

/// Trait implemented by display drivers.
pub trait DisplayDriver {
    /// Return the screen geometry (logical size + scan rotation).
    ///
    /// Applications use [`Screen::logical_size`] to size widgets;
    /// renderers and compositors consult [`Screen::rotation`] to map
    /// logical coordinates onto the physical framebuffer.
    fn screen(&self) -> Screen;

    /// Flush a rectangular region of pixels to the display.
    ///
    /// `area` and `colors` are in **logical** coordinates. The driver
    /// is responsible for rotating them into the physical framebuffer
    /// according to its [`Screen::rotation`].
    fn flush(&mut self, area: Rect, colors: &[Color]);

    /// Optional vertical sync hook.
    fn vsync(&mut self) {}
}

/// Dummy headless driver used for tests.
pub struct DummyDisplay;

impl DisplayDriver for DummyDisplay {
    fn screen(&self) -> Screen {
        Screen::landscape(0, 0)
    }
    fn flush(&mut self, _area: Rect, _colors: &[Color]) {}
}

/// In-memory framebuffer driver for tests and headless rendering.
pub struct BufferDisplay {
    /// Width of the framebuffer in pixels.
    pub width: usize,
    /// Height of the framebuffer in pixels.
    pub height: usize,
    /// Pixel buffer stored in row-major order.
    pub buffer: Vec<Color>,
}

impl BufferDisplay {
    /// Create a framebuffer with the specified dimensions.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            buffer: vec![Color(0, 0, 0, 255); width * height],
        }
    }
}

impl DisplayDriver for BufferDisplay {
    fn screen(&self) -> Screen {
        Screen::landscape(self.width as u32, self.height as u32)
    }
    fn flush(&mut self, area: Rect, colors: &[Color]) {
        for y in 0..area.height as usize {
            for x in 0..area.width as usize {
                let idx = (area.y as usize + y) * self.width + (area.x as usize + x);
                self.buffer[idx] = colors[y * area.width as usize + x];
            }
        }
    }
}
