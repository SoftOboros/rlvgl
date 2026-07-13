// SPDX-License-Identifier: MIT
//! Invisible automation hotspots for tagged simulator interactions.

use alloc::boxed::Box;

use rlvgl_core::{
    event::Event,
    renderer::Renderer,
    widget::{Rect, Widget},
};

type ActivateFn = Box<dyn FnMut()>;
type VisibleFn = Box<dyn Fn() -> bool>;

/// Transparent widget that triggers an action when tapped.
pub struct ActionHotspot {
    bounds: Rect,
    on_tap: ActivateFn,
    is_visible: VisibleFn,
}

impl ActionHotspot {
    /// Create an always-visible hotspot.
    pub fn new(bounds: Rect, on_tap: impl FnMut() + 'static) -> Self {
        Self {
            bounds,
            on_tap: Box::new(on_tap),
            is_visible: Box::new(|| true),
        }
    }

    /// Add a dynamic visibility predicate for automation queries.
    pub fn with_visibility(mut self, is_visible: impl Fn() -> bool + 'static) -> Self {
        self.is_visible = Box::new(is_visible);
        self
    }

    fn visible_bounds(&self) -> Rect {
        if (self.is_visible)() {
            self.bounds
        } else {
            Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            }
        }
    }
}

impl Widget for ActionHotspot {
    fn bounds(&self) -> Rect {
        self.visible_bounds()
    }

    fn draw(&self, _renderer: &mut dyn Renderer) {}

    fn handle_event(&mut self, event: &Event) -> bool {
        if !(self.is_visible)() {
            return false;
        }

        if let Event::PressRelease { x, y } = event {
            let bounds = self.bounds;
            if *x >= bounds.x
                && *x < bounds.x + bounds.width
                && *y >= bounds.y
                && *y < bounds.y + bounds.height
            {
                (self.on_tap)();
                return true;
            }
        }

        false
    }
}
