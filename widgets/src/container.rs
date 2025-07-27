use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Rect, Widget};

pub struct Container {
    bounds: Rect,
    pub style: Style,
}

impl Container {
    pub fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            style: Style::default(),
        }
    }
}

impl Widget for Container {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        renderer.fill_rect(self.bounds, self.style.bg_color);
    }

    fn handle_event(&mut self, _event: &Event) {}
}
