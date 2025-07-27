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
