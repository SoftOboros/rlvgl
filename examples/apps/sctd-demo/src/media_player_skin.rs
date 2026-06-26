// SPDX-License-Identifier: MIT
//! Media Player skin — mounts the `rlvgl-creator qt emit`-generated,
//! Bolero-composed media-player widget tree ([`crate::media_player_gen`]) into
//! the SCTD demo's MP slot.
//!
//! The tree is produced end-to-end by the QML ingest pipeline from the scjson
//! tutorial's `SkodaBoleroInfotainment/Qml/Media/FrameMedia.qml`: the parser,
//! the QT-07 asset harvest, the `Image` → `rlvgl_widgets::Image` lowering, the
//! QT-03c sibling-relative anchor solver, and cross-component instantiation all
//! contribute. Artwork is the vendored Bolero RLE set in [`crate::qt_assets`].
//!
//! This wrapper adds only a visibility gate so the controller can show/hide the
//! skin exactly like the Machine Panel / Philosophers Table, and lays the tree
//! out across the 720×480 content area (left of the right-edge selector strip).

use core::cell::RefCell;

use rlvgl_core::WidgetNode;
use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Rect, Widget};

use crate::media_player_gen;

/// Visibility-gated wrapper around the emitted media-player widget tree.
pub struct MediaPlayerSkin {
    bounds: Rect,
    /// The emitted, composed media-player tree, built once at construction.
    node: RefCell<WidgetNode>,
    /// Whether the skin is currently shown (MP slot selected).
    visible: bool,
}

impl MediaPlayerSkin {
    /// Build the skin from the emitted `build_screen`, laid out at `bounds`
    /// (the 720×480 content area). Starts hidden; the controller calls
    /// [`set_visible`](Self::set_visible) when the MP slot is selected.
    pub fn new(bounds: Rect) -> Self {
        // The emitted tree is self-positioning from `bounds` via the QT-03c
        // anchor solver; the returned state/bindings are unused here (the skin
        // is currently static — reactive artwork swap is a follow-up).
        let (node, _state, _bindings) = media_player_gen::build_screen(bounds);
        Self {
            bounds,
            node: RefCell::new(node),
            visible: false,
        }
    }

    /// Show or hide the skin. When hidden, `draw` is a no-op.
    pub fn set_visible(&mut self, v: bool) {
        self.visible = v;
    }

    /// Whether the skin is currently shown.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_visible(&self) -> bool {
        self.visible
    }
}

impl Widget for MediaPlayerSkin {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if self.visible {
            self.node.borrow().draw(renderer);
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if !self.visible {
            return false;
        }
        // Forward taps into the emitted tree (e.g. ClickArea/Button regions).
        self.node.borrow_mut().dispatch_event(event)
    }
}
