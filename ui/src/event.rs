// SPDX-License-Identifier: MIT OR Apache-2.0
//! Event hook helpers for rlvgl-ui widgets.
//!
//! Provides builder-style `on_click` and `on_change` APIs.

use alloc::boxed::Box;
use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Rect, Widget},
};
use rlvgl_widgets::{button::Button as BaseButton, slider::Slider as BaseSlider};

/// Extension trait adding a fluent `on_click` method to widgets.
pub trait OnClick {
    /// Attach a click handler executed when the widget is released.
    fn on_click<F: FnMut(&mut Self) + 'static>(self, handler: F) -> Self;
}

impl OnClick for BaseButton {
    fn on_click<F: FnMut(&mut Self) + 'static>(mut self, handler: F) -> Self {
        self.set_on_click(handler);
        self
    }
}

/// Slider widget with change callback support.
pub struct Slider {
    inner: BaseSlider,
    on_change: Option<Box<dyn FnMut(i32)>>,
}

impl Slider {
    /// Create a new slider with the provided range.
    pub fn new(bounds: Rect, min: i32, max: i32) -> Self {
        let inner = BaseSlider::new(bounds, min, max);
        Self {
            inner,
            on_change: None,
        }
    }

    /// Register a callback invoked whenever the slider value changes.
    pub fn on_change<F: FnMut(i32) + 'static>(mut self, handler: F) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// Current slider value.
    pub fn value(&self) -> i32 {
        self.inner.value()
    }

    /// Set the slider value, clamped to the valid range.
    pub fn set_value(&mut self, val: i32) {
        self.inner.set_value(val);
    }

    /// Immutable access to the slider style.
    pub fn style(&self) -> &rlvgl_core::style::Style {
        &self.inner.style
    }

    /// Mutable access to the slider style.
    pub fn style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        &mut self.inner.style
    }
}

impl Widget for Slider {
    fn bounds(&self) -> Rect {
        self.inner.bounds()
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        self.inner.draw(renderer);
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        let before = self.inner.value();
        let handled = self.inner.handle_event(event);
        let after = self.inner.value();
        if handled && after != before {
            if let Some(cb) = self.on_change.as_mut() {
                cb(after);
            }
        }
        handled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::rc::Rc;
    use core::cell::Cell;
    use rlvgl_core::{event::Event, widget::Rect};
    use rlvgl_widgets::button::Button;

    #[test]
    fn button_on_click_executes() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 20,
        };
        let clicked = Rc::new(Cell::new(false));
        let c = clicked.clone();
        let mut btn = Button::new("ok", bounds).on_click(move |_| c.set(true));
        let event = Event::PointerUp { x: 5, y: 5 };
        btn.handle_event(&event);
        assert!(clicked.get());
    }

    #[test]
    fn slider_on_change_executes() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 10,
        };
        let value = Rc::new(Cell::new(0));
        let v = value.clone();
        let mut slider = Slider::new(bounds, 0, 10).on_change(move |x| v.set(x));
        let event = Event::PointerUp { x: 50, y: 5 };
        slider.handle_event(&event);
        assert_ne!(value.get(), 0);
    }
}
