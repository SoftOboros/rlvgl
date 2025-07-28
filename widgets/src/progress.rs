use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

/// Simple progress bar widget.
pub struct ProgressBar {
    bounds: Rect,
    pub style: Style,
    pub bar_color: Color,
    min: i32,
    max: i32,
    value: i32,
}

impl ProgressBar {
    /// Create a new progress bar with a value range.
    pub fn new(bounds: Rect, min: i32, max: i32) -> Self {
        Self {
            bounds,
            style: Style::default(),
            bar_color: Color(0, 0, 0),
            min,
            max,
            value: min,
        }
    }

    /// Current progress value.
    pub fn value(&self) -> i32 {
        self.value
    }

    /// Set the progress value, clamped to the configured range.
    pub fn set_value(&mut self, val: i32) {
        self.value = val.clamp(self.min, self.max);
    }

    /// Convert the current value to a filled width in pixels.
    fn width_from_value(&self) -> i32 {
        let range = self.max - self.min;
        if range == 0 {
            return 0;
        }
        let ratio = (self.value - self.min) as f32 / range as f32;
        (ratio * self.bounds.width as f32) as i32
    }
}

impl Widget for ProgressBar {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        renderer.fill_rect(self.bounds, self.style.bg_color);

        let bar_width = self.width_from_value();
        let bar_rect = Rect {
            x: self.bounds.x,
            y: self.bounds.y,
            width: bar_width,
            height: self.bounds.height,
        };
        renderer.fill_rect(bar_rect, self.bar_color);
    }

    /// Progress bars are display only and ignore events.
    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}
