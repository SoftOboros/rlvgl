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

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

use rlvgl_core::WidgetNode;
use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Rect, Widget};

use crate::media_player_gen::{self, Binding, ScreenState};

/// The QML transport Play button's synthesised id (QT-Repeater expansion):
/// the second model item (`MediaFunc.Play`). Its node carries this tag, so the
/// skin can locate its bounds to route a tap into the machine.
const PLAY_BUTTON_TAG: &str = "__rep_btn_1";

/// The single machine event the Play button dispatches. The machine owns the
/// play↔pause decision (QT-05g; see `media_player_normalized.scxml`), so the
/// skin sends one toggle event — no predicate branch in glue.
const PLAY_PAUSE_EVENT: &str = "Inp.Media.PlayPause";

/// Visibility-gated wrapper around the emitted media-player widget tree, wired
/// to its istate (linkage-v2) `media_player::Machine` (QT-05g).
///
/// The Play button's icon is driven entirely by the emitted
/// `Binding::Predicate` over `machine.is_active("mediaPlaying")`: a tap steps
/// the machine and `refresh_bindings` swaps the artwork Play↔Pause. The skin
/// owns the machine returned by `build_screen` and forwards a tap in the
/// Play-button region as the toggle event.
pub struct MediaPlayerSkin {
    bounds: Rect,
    /// The emitted, composed media-player tree, built once at construction.
    node: RefCell<WidgetNode>,
    /// QT-04b root-property state threaded through the tree.
    state: Rc<RefCell<ScreenState>>,
    /// The istate (linkage-v2) machine driving the reactive artwork.
    machine: Rc<RefCell<media_player::Machine>>,
    /// Reactive bindings (QT-05g `Binding::Predicate` + any labels).
    bindings: Vec<Binding>,
    /// Bounds of the Play button (tag [`PLAY_BUTTON_TAG`]), resolved once from
    /// the built tree; `None` if the tag is absent.
    play_bounds: Option<Rect>,
    /// Whether the skin is currently shown (MP slot selected).
    visible: bool,
}

/// Recursively find the bounds of the first node tagged `tag`.
fn find_bounds_by_tag(node: &WidgetNode, tag: &str) -> Option<Rect> {
    if node.tag == Some(tag) {
        return Some(node.widget.borrow().bounds());
    }
    node.children
        .iter()
        .find_map(|child| find_bounds_by_tag(child, tag))
}

impl MediaPlayerSkin {
    /// Build the skin from the emitted `build_screen`, laid out at `bounds`
    /// (the 720×480 content area). Starts hidden; the controller calls
    /// [`set_visible`](Self::set_visible) when the MP slot is selected.
    pub fn new(bounds: Rect) -> Self {
        // The emitted tree is self-positioning from `bounds` via the QT-03c
        // anchor solver. `build_screen` constructs + `start()`s the machine
        // (linkage v2) and returns the reactive bindings; the skin owns them.
        let (node, state, machine, bindings) = media_player_gen::build_screen(bounds);
        let play_bounds = find_bounds_by_tag(&node, PLAY_BUTTON_TAG);
        // `build_screen` constructs + `start()`s the machine, leaving it in
        // `mediaPlayerIdle`. Seed it past idle to the transport state
        // (`mediaStopped`) so the Play button is live — mirrors the
        // `MediaPlayerAdapter` config seeding (SCTD-03 §8). This is machine
        // initialisation, not a predicate branch.
        {
            let mut m = machine.borrow_mut();
            m.step("Inp.Media.Ready", media_player::Value::Undefined);
            m.step("Inp.Media.ValidSource", media_player::Value::Undefined);
        }
        // Apply the initial machine-driven artwork (Play at rest — stopped).
        media_player_gen::refresh_bindings(&state, &machine, &bindings);
        Self {
            bounds,
            node: RefCell::new(node),
            state,
            machine,
            bindings,
            play_bounds,
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

    /// Toggle the machine's transport (Play↔Pause) and re-apply the reactive
    /// bindings, so the Play button's artwork tracks `is_active("mediaPlaying")`.
    fn toggle_play_pause(&self) {
        self.machine
            .borrow_mut()
            .step(PLAY_PAUSE_EVENT, media_player::Value::Undefined);
        media_player_gen::refresh_bindings(&self.state, &self.machine, &self.bindings);
    }

    /// Bounds of the Play button (for the pixel gate).
    #[cfg(test)]
    fn play_bounds(&self) -> Option<Rect> {
        self.play_bounds
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
        // A tap in the Play-button region toggles the machine (which swaps the
        // icon via the predicate binding). `PressRelease` is the committed tap
        // trigger (see rlvgl CLAUDE.md runtime protocol).
        if let Event::PressRelease { x, y } | Event::PointerUp { x, y } = *event
            && let Some(pb) = self.play_bounds
            && x >= pb.x
            && x < pb.x + pb.width
            && y >= pb.y
            && y < pb.y + pb.height
        {
            self.toggle_play_pause();
            return true;
        }
        // Other taps still forward into the emitted tree (ClickArea regions).
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

    /// Count pixels inside `rect` that differ between two framebuffers.
    fn region_diff(a: &Framebuffer, b: &Framebuffer, rect: Rect) -> u32 {
        let mut n = 0;
        for y in rect.y..rect.y + rect.height {
            for x in rect.x..rect.x + rect.width {
                if x < 0 || y < 0 || x >= a.w || y >= a.h {
                    continue;
                }
                let i = (y * a.w + x) as usize;
                if a.px[i] != b.px[i] {
                    n += 1;
                }
            }
        }
        n
    }

    /// QT-05g reactive gate: tapping the Play button steps the machine and the
    /// predicate binding swaps the artwork (Play → Pause). The pixels in the
    /// Play-button region MUST change materially.
    #[test]
    fn play_button_artwork_swaps_on_tap() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 720,
            height: 480,
        };
        let mut skin = MediaPlayerSkin::new(bounds);
        skin.set_visible(true);
        let pb = skin
            .play_bounds()
            .expect("Play button (__rep_btn_1) must be present in the emitted tree");

        // Frame 1: at rest (machine stopped → Play icon).
        let mut fb1 = Framebuffer::new(720, 480);
        skin.draw(&mut fb1);

        // Tap the centre of the Play button → step(PlayPause) → playing.
        let consumed = skin.handle_event(&Event::PressRelease {
            x: pb.x + pb.width / 2,
            y: pb.y + pb.height / 2,
        });
        assert!(consumed, "tap in the Play-button region must be consumed");

        // Frame 2: after the toggle (machine playing → Pause icon).
        let mut fb2 = Framebuffer::new(720, 480);
        skin.draw(&mut fb2);

        let changed = region_diff(&fb1, &fb2, pb);
        let area = (pb.width * pb.height).max(1) as u32;
        // The Play and Pause glyphs differ across a large fraction of the 48×48
        // icon; require a clear, non-noise change.
        assert!(
            changed * 100 / area >= 10,
            "Play→Pause swap changed only {changed}/{area} px in the button region \
             — the predicate binding did not drive the artwork"
        );

        // Tapping again toggles back (playing → paused → Play icon); the region
        // must change again (not latched).
        skin.handle_event(&Event::PressRelease {
            x: pb.x + pb.width / 2,
            y: pb.y + pb.height / 2,
        });
        let mut fb3 = Framebuffer::new(720, 480);
        skin.draw(&mut fb3);
        assert!(
            region_diff(&fb2, &fb3, pb) > 0,
            "second tap (Pause→Play) must change the Play-button artwork again"
        );
    }
}
