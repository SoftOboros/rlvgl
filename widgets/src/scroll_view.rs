//! Scrollable viewport container (REND initiative).
//!
//! [`ScrollView`] owns children positioned in **content space** (origin at
//! the content's top-left, independent of scroll state), exposes a
//! per-pixel vertical scroll offset, and renders children translated and
//! clipped to its own bounds via
//! [`ClipRenderer`](rlvgl_core::renderer::ClipRenderer) — partially
//! visible rows crop cleanly at the viewport's edges instead of bleeding
//! over siblings.
//!
//! Normative contract: `docs/concepts/REND-00-CONCEPTS.md` §6. No kinetic
//! scrolling and no scroll gesture recognition — consumers drive
//! [`scroll_to`](ScrollView::scroll_to) / [`scroll_by`](ScrollView::scroll_by)
//! from their own input handling, and draw custom scrollbars from the
//! [`viewport`](ScrollView::viewport) / [`content_height`](ScrollView::content_height)
//! / [`scroll_y`](ScrollView::scroll_y) / [`max_scroll`](ScrollView::max_scroll)
//! query seam.

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

use rlvgl_core::draw::draw_widget_bg;
use rlvgl_core::event::Event;
use rlvgl_core::renderer::{ClipRenderer, Renderer};
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Rect, Widget};

/// Width in pixels of the optional built-in scrollbar thumb.
const SCROLLBAR_WIDTH: i32 = 4;
/// Gap between the scrollbar and the viewport's right edge.
const SCROLLBAR_MARGIN: i32 = 2;
/// Minimum thumb height so it stays grabbable/visible on long content.
const SCROLLBAR_MIN_THUMB: i32 = 8;

/// Vertical scrolling viewport over taller content (REND-00 §6).
///
/// Children live in content space: a child with bounds `y = 0` sits at
/// the top of the *content*, which coincides with the top of the viewport
/// only while `scroll_y == 0`.
pub struct ScrollView {
    bounds: Rect,
    content_height: i32,
    scroll_y: i32,
    children: Vec<Rc<RefCell<dyn Widget>>>,
    /// Visual style of the viewport background.
    pub style: Style,
    /// Draw the optional built-in proportional scrollbar thumb
    /// (default `false`; the query seam is the supported contract for
    /// custom scrollbars).
    pub show_scrollbar: bool,
    /// Set when the scroll offset changed since the last
    /// [`take_dirty`](Self::take_dirty).
    dirty: bool,
}

impl ScrollView {
    /// Create a viewport at `bounds` over `content_height` pixels of
    /// content. A content height ≤ the viewport height simply never
    /// scrolls.
    pub fn new(bounds: Rect, content_height: i32) -> Self {
        Self {
            bounds,
            content_height: content_height.max(0),
            scroll_y: 0,
            children: Vec::new(),
            style: Style::default(),
            show_scrollbar: false,
            dirty: false,
        }
    }

    /// Add a child widget positioned in content space.
    pub fn add_child(&mut self, child: Rc<RefCell<dyn Widget>>) {
        self.children.push(child);
    }

    /// Replace the content extent (e.g. after adding rows). The current
    /// offset is re-clamped; a clamp that moves it marks the view dirty.
    pub fn set_content_height(&mut self, content_height: i32) {
        self.content_height = content_height.max(0);
        let clamped = self.scroll_y.clamp(0, self.max_scroll());
        if clamped != self.scroll_y {
            self.scroll_y = clamped;
            self.dirty = true;
        }
    }

    /// The viewport rect (the widget's own bounds, screen space).
    pub fn viewport(&self) -> Rect {
        self.bounds
    }

    /// Total content extent in pixels.
    pub fn content_height(&self) -> i32 {
        self.content_height
    }

    /// Current per-pixel scroll offset (`0..=max_scroll`).
    pub fn scroll_y(&self) -> i32 {
        self.scroll_y
    }

    /// Maximum reachable scroll offset.
    pub fn max_scroll(&self) -> i32 {
        (self.content_height - self.bounds.height).max(0)
    }

    /// Scroll to an absolute offset, clamped to `0..=max_scroll`.
    /// Marks the view dirty only when the effective offset changes.
    pub fn scroll_to(&mut self, y: i32) {
        let clamped = y.clamp(0, self.max_scroll());
        if clamped != self.scroll_y {
            self.scroll_y = clamped;
            self.dirty = true;
        }
    }

    /// Scroll by a signed per-pixel delta (clamped; see
    /// [`scroll_to`](Self::scroll_to)).
    pub fn scroll_by(&mut self, dy: i32) {
        self.scroll_to(self.scroll_y + dy);
    }

    /// The region invalidated by scroll-offset changes since the last
    /// call: the viewport rect, once, then `None` until the next change
    /// (REND-00 §6.6 — a scrolled panel invalidates its viewport, not
    /// the whole frame).
    pub fn take_dirty(&mut self) -> Option<Rect> {
        if self.dirty {
            self.dirty = false;
            Some(self.bounds)
        } else {
            None
        }
    }

    /// Bounds of the built-in scrollbar thumb, if it would be drawn.
    fn scrollbar_thumb(&self) -> Option<Rect> {
        if !self.show_scrollbar || self.content_height <= self.bounds.height {
            return None;
        }
        let track_h = self.bounds.height;
        let thumb_h = ((track_h as i64 * track_h as i64) / self.content_height as i64) as i32;
        let thumb_h = thumb_h.clamp(SCROLLBAR_MIN_THUMB, track_h);
        let travel = track_h - thumb_h;
        let max = self.max_scroll();
        let offset = if max > 0 {
            ((self.scroll_y as i64 * travel as i64) / max as i64) as i32
        } else {
            0
        };
        Some(Rect {
            x: self.bounds.x + self.bounds.width - SCROLLBAR_WIDTH - SCROLLBAR_MARGIN,
            y: self.bounds.y + offset,
            width: SCROLLBAR_WIDTH,
            height: thumb_h,
        })
    }

    /// Translate a screen-space point into content space, or `None` when
    /// it falls outside the viewport.
    fn to_content(&self, x: i32, y: i32) -> Option<(i32, i32)> {
        let b = self.bounds;
        if x < b.x || x >= b.x + b.width || y < b.y || y >= b.y + b.height {
            return None;
        }
        Some((x - b.x, y - b.y + self.scroll_y))
    }

    /// Translate a pointer-family event into content space, or `None`
    /// when the event is outside the viewport (not delivered) or not a
    /// pointer event (pass through untranslated).
    fn translate_pointer(&self, event: &Event) -> Option<Option<Event>> {
        let remap = |x: i32, y: i32, build: fn(i32, i32) -> Event| -> Option<Event> {
            self.to_content(x, y).map(|(cx, cy)| build(cx, cy))
        };
        let translated = match *event {
            Event::PointerDown { x, y } => remap(x, y, |x, y| Event::PointerDown { x, y }),
            Event::PointerUp { x, y } => remap(x, y, |x, y| Event::PointerUp { x, y }),
            Event::PointerMove { x, y } => remap(x, y, |x, y| Event::PointerMove { x, y }),
            Event::PressDown { x, y } => remap(x, y, |x, y| Event::PressDown { x, y }),
            Event::PressRelease { x, y } => remap(x, y, |x, y| Event::PressRelease { x, y }),
            Event::DoubleTap { x, y } => remap(x, y, |x, y| Event::DoubleTap { x, y }),
            _ => return None,
        };
        Some(translated)
    }
}

impl Widget for ScrollView {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        draw_widget_bg(renderer, self.bounds, &self.style);
        {
            // Children draw in content space; the adapter shifts them to
            // screen space and crops to the viewport (REND-00 §6.8).
            let mut clipped = ClipRenderer::with_offset(
                renderer,
                self.bounds,
                self.bounds.x,
                self.bounds.y - self.scroll_y,
            );
            for child in &self.children {
                child.borrow().draw(&mut clipped);
            }
        }
        if let Some(thumb) = self.scrollbar_thumb() {
            renderer.fill_rect(thumb, self.style.border_color);
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        match self.translate_pointer(event) {
            // Pointer event inside the viewport: forward translated.
            Some(Some(translated)) => {
                for child in &self.children {
                    if child.borrow_mut().handle_event(&translated) {
                        return true;
                    }
                }
                false
            }
            // Pointer event outside the viewport: children never see it.
            Some(None) => false,
            // Non-pointer event (Tick, keys, …): pass through unchanged.
            None => {
                for child in &self.children {
                    if child.borrow_mut().handle_event(event) {
                        return true;
                    }
                }
                false
            }
        }
    }
}
