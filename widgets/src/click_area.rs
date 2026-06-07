//! Transparent click-area widget used as the rlvgl-side analogue of
//! QML's `MouseArea`. Renders nothing; reports clicks through an
//! optional callback the caller registers with [`Self::set_on_click`].

use alloc::boxed::Box;

use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Rect, Widget};

type ClickHandler = Box<dyn FnMut(&mut ClickArea)>;

/// Transparent rectangular hit area with an optional click callback.
///
/// Designed for the QML `MouseArea` lowering shipped by phase QT-04d
/// (`docs/qt-support/04d-mousearea.md`). The widget intentionally
/// has no visual: its [`Widget::draw`] is a no-op so the area never
/// occludes its parent's pixels. Hit-testing happens on
/// [`Event::PressRelease`].
pub struct ClickArea {
    bounds: Rect,
    on_click: Option<ClickHandler>,
}

impl ClickArea {
    /// Create a click area covering `bounds`.
    pub fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            on_click: None,
        }
    }

    /// Register a callback invoked when a click is released inside
    /// `bounds`. The callback receives `&mut self` so it can mutate
    /// the click area itself (e.g. resize after a click) — though
    /// QT-04d's emitted closures typically only mutate `ScreenState`
    /// captured by reference.
    pub fn set_on_click<F: FnMut(&mut Self) + 'static>(&mut self, handler: F) {
        self.on_click = Some(Box::new(handler));
    }

    fn inside_bounds(&self, x: i32, y: i32) -> bool {
        let b = self.bounds;
        x >= b.x && x < b.x + b.width && y >= b.y && y < b.y + b.height
    }
}

impl Widget for ClickArea {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    /// No-op — the click area is transparent.
    fn draw(&self, _renderer: &mut dyn Renderer) {}

    fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::PressRelease { x, y } if self.inside_bounds(*x, *y) => {
                if let Some(mut cb) = self.on_click.take() {
                    cb(self);
                    self.on_click = Some(cb);
                }
                true
            }
            _ => false,
        }
    }
}
