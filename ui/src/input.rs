// SPDX-License-Identifier: MIT
//! Input and textarea components for rlvgl-ui.
//!
//! These wrappers provide simple text fields backed by the base label widget.

use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Rect, Widget},
};
use rlvgl_widgets::label::Label;

/// Single-line text input component.
pub struct Input {
    inner: Label,
    on_change: Option<Box<dyn FnMut(&str)>>,
}

impl Input {
    /// Create a new input with the provided initial value and bounds.
    pub fn new(text: &str, bounds: Rect) -> Self {
        Self {
            inner: Label::new(text, bounds),
            on_change: None,
        }
    }

    /// Register a change handler invoked when [`set_text`] is called.
    pub fn on_change<F: FnMut(&str) + 'static>(mut self, handler: F) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// Immutable access to the input style.
    pub fn style(&self) -> &rlvgl_core::style::Style {
        &self.inner.style
    }

    /// Mutable access to the input style.
    pub fn style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        &mut self.inner.style
    }

    /// Update the input text and trigger the change handler if present.
    pub fn set_text(&mut self, text: &str) {
        self.inner.set_text(text);
        if let Some(cb) = self.on_change.as_mut() {
            cb(self.inner.text());
        }
    }

    /// Retrieve the current input text.
    pub fn text(&self) -> &str {
        self.inner.text()
    }
}

impl Widget for Input {
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

/// Multi-line textarea component.
pub struct Textarea {
    inner: Input,
}

impl Textarea {
    /// Create a new textarea with the provided text and bounds.
    pub fn new(text: &str, bounds: Rect) -> Self {
        Self {
            inner: Input::new(text, bounds),
        }
    }

    /// Register a change handler invoked when the text updates.
    pub fn on_change<F: FnMut(&str) + 'static>(mut self, handler: F) -> Self {
        self.inner = self.inner.on_change(handler);
        self
    }

    /// Immutable access to the textarea style.
    pub fn style(&self) -> &rlvgl_core::style::Style {
        self.inner.style()
    }

    /// Mutable access to the textarea style.
    pub fn style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        self.inner.style_mut()
    }

    /// Update the textarea text.
    pub fn set_text(&mut self, text: &str) {
        self.inner.set_text(text);
    }

    /// Retrieve the textarea content.
    pub fn text(&self) -> &str {
        self.inner.text()
    }
}

impl Widget for Textarea {
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
    fn input_sets_text_and_calls_handler() {
        let called = Rc::new(Cell::new(false));
        let flag = called.clone();
        let mut input = Input::new(
            "hi",
            Rect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            },
        )
        .on_change(move |_| flag.set(true));
        input.set_text("new");
        assert_eq!(input.text(), "new");
        assert!(called.get());
    }

    #[test]
    fn textarea_wraps_input() {
        let mut area = Textarea::new(
            "a",
            Rect {
                x: 0,
                y: 0,
                width: 10,
                height: 20,
            },
        );
        area.set_text("b");
        assert_eq!(area.text(), "b");
    }
}
