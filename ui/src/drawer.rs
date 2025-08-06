// SPDX-License-Identifier: MIT OR Apache-2.0
//! Drawer component for rlvgl-ui.
//!
//! Provides a side panel container for navigation or menus.

use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Rect, Widget},
};
use rlvgl_widgets::{container::Container, label::Label};

/// Sliding side panel displaying arbitrary content.
pub struct Drawer {
    container: Container,
    label: Label,
}

impl Drawer {
    /// Create a new drawer with the provided title and bounds.
    pub fn new(text: &str, bounds: Rect) -> Self {
        let container = Container::new(bounds);
        let label = Label::new(text, bounds);
        Self { container, label }
    }

    /// Immutable access to the drawer container style.
    pub fn style(&self) -> &rlvgl_core::style::Style {
        &self.container.style
    }

    /// Mutable access to the drawer container style.
    pub fn style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        &mut self.container.style
    }

    /// Update the drawer title text.
    pub fn set_text(&mut self, text: &str) {
        self.label.set_text(text);
    }

    /// Retrieve the drawer title text.
    pub fn text(&self) -> &str {
        self.label.text()
    }
}

impl Widget for Drawer {
    fn bounds(&self) -> Rect {
        self.container.bounds()
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        self.container.draw(renderer);
        self.label.draw(renderer);
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        self.label.handle_event(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rlvgl_core::widget::Rect;

    #[test]
    fn drawer_updates_text() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 30,
            height: 10,
        };
        let mut d = Drawer::new("menu", bounds);
        d.set_text("nav");
        assert_eq!(d.text(), "nav");
    }
}
