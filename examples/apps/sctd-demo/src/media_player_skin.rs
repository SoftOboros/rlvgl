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

#[cfg(test)]
mod pixel_gate {
    //! Pixel-level render gate for the media-player skin
    //! (QT-MEDIA-PLAYER-RETROSPECTIVE.md §6.3, binding precondition).
    //!
    //! The existing demo tests draw through a `NullRenderer` (no pixels),
    //! so they assert "does not panic" — they could not catch the
    //! all-white-screen regression that the v14→v15 effort chased
    //! (retrospective §2 divergence #1). This gate renders the skin into a
    //! real alpha-blended framebuffer and asserts a white/art histogram:
    //! the post-fix content region is ~0.4 % white / ~90 % artwork, so a
    //! regression to the white-screen bug (≈90 % white) fails here.

    use super::*;
    use rlvgl_core::renderer::Renderer;
    use rlvgl_core::widget::Color;

    /// Minimal alpha-blending framebuffer that captures every painted
    /// pixel. `fill_rect` is the single low-level sink: the core's default
    /// `blit_image` → `draw_pixels` → `fill_rect` decomposition routes all
    /// image artwork (the Bolero background + transport icons) through it,
    /// so capturing `fill_rect` captures the whole rendered frame.
    struct Framebuffer {
        w: i32,
        h: i32,
        px: Vec<Color>,
    }

    impl Framebuffer {
        fn new(w: i32, h: i32) -> Self {
            Self {
                w,
                h,
                px: vec![Color(0, 0, 0, 255); (w * h) as usize],
            }
        }

        /// Source-over alpha blend (matches the device showing the Bolero
        /// background through magenta-keyed transparent icon pixels).
        fn blend(&mut self, x: i32, y: i32, c: Color) {
            if x < 0 || y < 0 || x >= self.w || y >= self.h {
                return;
            }
            let a = c.3 as u32;
            if a == 0 {
                return;
            }
            let i = (y * self.w + x) as usize;
            if a == 255 {
                self.px[i] = c;
                return;
            }
            let d = self.px[i];
            let mix = |s: u8, dd: u8| ((s as u32 * a + dd as u32 * (255 - a)) / 255) as u8;
            self.px[i] = Color(mix(c.0, d.0), mix(c.1, d.1), mix(c.2, d.2), 255);
        }
    }

    impl Renderer for Framebuffer {
        fn fill_rect(&mut self, rect: Rect, color: Color) {
            for y in rect.y..rect.y + rect.height {
                for x in rect.x..rect.x + rect.width {
                    self.blend(x, y, color);
                }
            }
        }
        // Text is not part of the artwork histogram; the skin's reactive
        // content is imagery. A no-op keeps the gate focused.
        fn draw_text(&mut self, _pos: (i32, i32), _text: &str, _color: Color) {}
    }

    /// (white_fraction, art_fraction) over the framebuffer. "white" =
    /// near-(255,255,255); "art" = neither near-white nor near-black, i.e.
    /// the colourful Bolero photo + icon pixels.
    fn histogram(fb: &Framebuffer) -> (f64, f64) {
        let total = fb.px.len() as f64;
        let mut white = 0u32;
        let mut art = 0u32;
        for c in &fb.px {
            let (r, g, b) = (c.0 as i32, c.1 as i32, c.2 as i32);
            let near_white = r > 240 && g > 240 && b > 240;
            let near_black = r.max(g).max(b) < 16;
            if r > 250 && g > 250 && b > 250 {
                white += 1;
            }
            if !near_white && !near_black {
                art += 1;
            }
        }
        (white as f64 / total, art as f64 / total)
    }

    /// Baseline gate: the static skin renders the Bolero background + control
    /// surface — NOT an all-white screen.
    #[test]
    fn media_player_skin_render_is_not_white_and_has_artwork() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 720,
            height: 480,
        };
        let mut skin = MediaPlayerSkin::new(bounds);
        skin.set_visible(true);
        let mut fb = Framebuffer::new(720, 480);
        skin.draw(&mut fb);

        let (white_frac, art_frac) = histogram(&fb);
        // Retrospective measured: post-fix content region ~0.4 % white,
        // ~90 % artwork. The white-screen bug was ≈90 % white.
        assert!(
            white_frac < 0.25,
            "MP skin rendered mostly white ({white_frac:.3}) — the white-screen regression"
        );
        assert!(
            art_frac > 0.40,
            "MP skin shows little artwork ({art_frac:.3}) — background/controls missing"
        );
    }
}
