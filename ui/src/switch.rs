// SPDX-License-Identifier: MIT
//! Switch component with change callbacks for rlvgl-ui.
//!
//! Wraps the base switch widget and exposes a builder-style `on_change`
//! handler fired whenever the on/off state toggles.

use alloc::boxed::Box;
use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Rect, Widget},
};
use rlvgl_widgets::switch::Switch as BaseSwitch;

/// Switch widget with optional change callback.
pub struct Switch {
    inner: BaseSwitch,
    on_change: Option<Box<dyn FnMut(bool)>>,
}

impl Switch {
    /// Create a new switch.
    pub fn new(bounds: Rect) -> Self {
        Self {
            inner: BaseSwitch::new(bounds),
            on_change: None,
        }
    }

    /// Attach a callback invoked when the state changes.
    pub fn on_change<F: FnMut(bool) + 'static>(mut self, handler: F) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// Return whether the switch is currently on.
    pub fn is_on(&self) -> bool {
        self.inner.is_on()
    }

    /// Programmatically set the on/off state.
    pub fn set_on(&mut self, value: bool) {
        self.inner.set_on(value);
    }

    /// Immutable access to the switch style.
    pub fn style(&self) -> &rlvgl_core::style::Style {
        &self.inner.style
    }

    /// Mutable access to the switch style.
    pub fn style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        &mut self.inner.style
    }
}

impl Widget for Switch {
    fn bounds(&self) -> Rect {
        self.inner.bounds()
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        self.inner.draw(renderer);
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        let before = self.inner.is_on();
        let handled = self.inner.handle_event(event);
        let after = self.inner.is_on();
        if handled && before != after {
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

    #[test]
    fn switch_on_change_triggers() {
        let rect = Rect {
            x: 0,
            y: 0,
            width: 40,
            height: 20,
        };
        let state = Rc::new(Cell::new(false));
        let s = state.clone();
        let mut sw = Switch::new(rect).on_change(move |v| s.set(v));
        let event = Event::PointerUp { x: 5, y: 5 };
        sw.handle_event(&event);
        assert!(state.get());
    }
}
