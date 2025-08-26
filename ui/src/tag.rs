// SPDX-License-Identifier: MIT
//! Tag component for rlvgl-ui.
//!
//! Built on top of the [`Button`](rlvgl_widgets::button::Button) widget to
//! provide lightweight controls for categorization or filters.

use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Rect, Widget},
};
use rlvgl_widgets::button::Button;

/// Clickable tag element that can notify when removed.
pub struct Tag {
    inner: Button,
}

impl Tag {
    /// Create a new tag with the provided label and bounds.
    pub fn new(text: &str, bounds: Rect) -> Self {
        let inner = Button::new(text, bounds);
        Self { inner }
    }

    /// Register a handler invoked when the tag is clicked.
    pub fn on_remove<F: FnMut() + 'static>(mut self, mut handler: F) -> Self {
        self.inner.set_on_click(move |_| handler());
        self
    }

    /// Immutable access to the tag style.
    pub fn style(&self) -> &rlvgl_core::style::Style {
        self.inner.style()
    }

    /// Mutable access to the tag style.
    pub fn style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        self.inner.style_mut()
    }

    /// Update the tag label.
    pub fn set_text(&mut self, text: &str) {
        self.inner.set_text(text);
    }

    /// Retrieve the current tag label.
    pub fn text(&self) -> &str {
        self.inner.text()
    }
}

impl Widget for Tag {
    fn bounds(&self) -> Rect {
        self.inner.bounds()
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        self.inner.draw(renderer);
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        self.inner.handle_event(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::rc::Rc;
    use core::cell::Cell;
    use rlvgl_core::widget::Rect;

    #[test]
    fn tag_triggers_remove() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        };
        let flag = Rc::new(Cell::new(false));
        let f = flag.clone();
        let mut t = Tag::new("rust", bounds).on_remove(move || f.set(true));
        let event = Event::PointerUp { x: 5, y: 5 };
        t.handle_event(&event);
        assert!(flag.get());
    }
}
