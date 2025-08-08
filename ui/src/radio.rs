// SPDX-License-Identifier: MIT
//! Radio component with change callbacks for rlvgl-ui.
//!
//! Wraps the base radio widget and exposes a builder-style `on_change`
//! handler that fires whenever the selection toggles.

use alloc::{boxed::Box, string::String};
use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Rect, Widget},
};
use rlvgl_widgets::radio::Radio as BaseRadio;

/// Radio button with optional change callback.
pub struct Radio {
    inner: BaseRadio,
    on_change: Option<Box<dyn FnMut(bool)>>,
}

impl Radio {
    /// Create a new radio button with the provided label text.
    pub fn new(text: impl Into<String>, bounds: Rect) -> Self {
        Self {
            inner: BaseRadio::new(text, bounds),
            on_change: None,
        }
    }

    /// Attach a callback invoked when the selected state changes.
    pub fn on_change<F: FnMut(bool) + 'static>(mut self, handler: F) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// Query whether the radio is currently selected.
    pub fn is_selected(&self) -> bool {
        self.inner.is_selected()
    }

    /// Programmatically set the selected state.
    pub fn set_selected(&mut self, value: bool) {
        self.inner.set_selected(value);
    }

    /// Immutable access to the radio style.
    pub fn style(&self) -> &rlvgl_core::style::Style {
        &self.inner.style
    }

    /// Mutable access to the radio style.
    pub fn style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        &mut self.inner.style
    }
}

impl Widget for Radio {
    fn bounds(&self) -> Rect {
        self.inner.bounds()
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        self.inner.draw(renderer);
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        let before = self.inner.is_selected();
        let handled = self.inner.handle_event(event);
        let after = self.inner.is_selected();
        if !handled || before == after {
            return handled;
        }
        if let Some(cb) = self.on_change.as_mut() {
            cb(after);
        }
        handled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::rc::Rc;
    use core::cell::Cell;

    #[test]
    fn radio_on_change_triggers() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 20,
        };
        let state = Rc::new(Cell::new(false));
        let s = state.clone();
        let mut radio = Radio::new("A", bounds).on_change(move |v| s.set(v));
        let event = Event::PointerUp { x: 5, y: 5 };
        radio.handle_event(&event);
        assert!(state.get());
    }
}
