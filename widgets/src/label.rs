use alloc::string::String;
use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

/// Simple text element.
pub struct Label {
    bounds: Rect,
    text: String,
    pub style: Style,
    pub text_color: Color,
}

impl Label {
    /// Create a new label with the provided text and bounds.
    pub fn new(text: impl Into<String>, bounds: Rect) -> Self {
        Self {
            bounds,
            text: text.into(),
            style: Style::default(),
            text_color: Color(0, 0, 0),
        }
    }
}

impl Widget for Label {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        renderer.fill_rect(self.bounds, self.style.bg_color);
        renderer.draw_text((self.bounds.x, self.bounds.y), &self.text, self.text_color);
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}
