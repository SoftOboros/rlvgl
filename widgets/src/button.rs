//! Interactive button widget with callback support.
use alloc::{boxed::Box, string::String};
use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Rect, Widget};

use crate::label::Label;
use rlvgl_core::style::Style;

type ClickHandler = Box<dyn FnMut(&mut Button)>;

/// Clickable button widget.
pub struct Button {
    label: Label,
    on_click: Option<ClickHandler>,
}

impl Button {
    /// Create a new button with the provided label text.
    pub fn new(text: impl Into<String>, bounds: Rect) -> Self {
        Self {
            label: Label::new(text, bounds),
            on_click: None,
        }
    }

    /// Immutable access to the button's style.
    pub fn style(&self) -> &Style {
        &self.label.style
    }

    /// Mutable access to the button's style.
    pub fn style_mut(&mut self) -> &mut Style {
        &mut self.label.style
    }

    /// Update the label displayed on the button.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.label.set_text(text);
    }

    /// Retrieve the current button label.
    pub fn text(&self) -> &str {
        self.label.text()
    }

    /// Register a callback invoked when the button is released.
    pub fn set_on_click<F: FnMut(&mut Self) + 'static>(&mut self, handler: F) {
        self.on_click = Some(Box::new(handler));
    }

    /// Check if the given coordinates are inside the button's bounds.
    fn inside_bounds(&self, x: i32, y: i32) -> bool {
        let b = self.label.bounds();
        x >= b.x && x < b.x + b.width && y >= b.y && y < b.y + b.height
    }
}

impl Widget for Button {
    fn bounds(&self) -> Rect {
        self.label.bounds()
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        self.label.draw(renderer);
    }

    /// Delegate pointer events and invoke the click handler when released.
    fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::PointerUp { x, y } if self.inside_bounds(*x, *y) => {
                if let Some(mut cb) = self.on_click.take() {
                    cb(self);
                    self.on_click = Some(cb);
                }
                return true;
            }
            _ => {}
        }
        false
    }
}
