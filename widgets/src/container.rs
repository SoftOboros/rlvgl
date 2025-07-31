//! Simple container grouping child widgets.
use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Rect, Widget};

/// Empty widget used to group child widgets and provide background styling.
pub struct Container {
    bounds: Rect,
    /// Visual style applied to the container background.
    pub style: Style,
}

impl Container {
    /// Create a new container with the specified bounds.
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

    /// Containers are currently passive and do not react to events.
    fn handle_event(&mut self, _event: &Event) -> bool {
        false
    }
}
