// SPDX-License-Identifier: MIT
//! Media Player skin — mounts the `rlvgl-creator qt emit`-generated,
//! Bolero-composed media-player widget tree ([`crate::media_player_gen`]) into
//! the disco demo's Play slot.
//!
//! The tree is produced end-to-end by the QML ingest pipeline from the scjson
//! tutorial's `SkodaBoleroInfotainment/Qml/Media/FrameMedia.qml`: the parser,
//! the QT-07 asset harvest, the `Image` → `rlvgl_widgets::Image` lowering, the
//! QT-03c sibling-relative anchor solver, and cross-component instantiation all
//! contribute. Artwork is the vendored Bolero RLE set in [`crate::qt_assets`].
//!
//! This wrapper adds only a visibility gate so the controller can show/hide the
//! skin exactly like the existing disco demo panels, and lays the tree out
//! across the content area left of the right-edge icon strip.

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;

use rlvgl_core::WidgetNode;
use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Rect, Widget};

use crate::media_player;
use crate::media_player_gen::{self, Binding, ScreenState};

const CLEAR_FRAMES: u8 = 3;

/// QT-05j — the QML-button-event → SCXML-input vocabulary map. The emitter
/// surfaces each tap target as `(node tag, raw QML "MediaFunc.*" event)` in
/// [`media_player_gen::BUTTON_TAP_EVENTS`] (lowered from the buttons'
/// `submitBtnSetupEvent("…")` handlers); this table — owned by the skin, the
/// role Bolero's C++ `submitBtnSetupEvent` plays — maps each QML event to the
/// machine input the SCXML consumes. `MediaFunc.Play` maps to the single
/// `Inp.Media.PlayPause` toggle (the machine owns the play↔pause decision,
/// QT-05g). Buttons with no entry here (e.g. `MediaFunc.Scan`, unmodelled) are
/// surfaced by the emitter but ignored — they dispatch nothing.
const MEDIA_FUNC_MAP: &[(&str, &str)] = &[
    ("MediaFunc.Play", "Inp.Media.PlayPause"),
    ("MediaFunc.Repeat", "Inp.Media.Repeat"),
    ("MediaFunc.Shuffle", "Inp.Media.Shuffle"),
    ("MediaFunc.Reverse", "Inp.Media.Prev"),
    ("MediaFunc.Forward", "Inp.Media.Next"),
];

/// QT-05k — the external-object text sources. The emitter surfaces each reactive
/// Label whose `text:` reads an external object property (e.g.
/// `audioPlayer.currentPlayUrlFileName`) as a `Binding::ExternalText` carrying
/// the verbatim key, plus a `(tag, key)` entry in
/// [`media_player_gen::EXTERNAL_TEXT_BINDINGS`]. This map — owned by the skin,
/// the role Bolero's C++ `audioPlayer` object plays — resolves each key to its
/// current value. The disco demo has no live media engine, so it supplies a
/// fixed representative now-playing caption; a real consumer would read its
/// player.
const EXTERNAL_TEXT_SOURCES: &[(&str, &str)] =
    &[("audioPlayer.currentPlayUrlFileName", "Bolero - Ravel.mp3")];

/// Resolve one external-text key to its current value (QT-05k consumer resolver).
/// Returns `None` for an unknown key, leaving that Label unchanged.
fn resolve_external_text(key: &str) -> Option<alloc::string::String> {
    use alloc::string::ToString;
    EXTERNAL_TEXT_SOURCES
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v.to_string())
}

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
    inner: RefCell<Option<MediaPlayerInner>>,
    /// Whether the skin is currently shown (MP slot selected).
    visible: bool,
    clear_countdown: u8,
}

struct MediaPlayerInner {
    /// The emitted, composed media-player tree, built once at construction.
    node: WidgetNode,
    /// QT-04b root-property state threaded through the tree.
    state: Rc<RefCell<ScreenState>>,
    /// The istate (linkage-v2) machine driving the reactive artwork.
    machine: Rc<RefCell<media_player::Machine>>,
    /// Reactive bindings (QT-05g `Binding::Predicate` / QT-05i `Binding::Chain`
    /// / QT-05h `Binding::Visibility` / QT-05k `Binding::ExternalText` + any
    /// labels).
    bindings: Vec<Binding>,
    /// Resolved tap targets: `(button bounds, machine event)` for every
    /// [`TAP_CONTROLS`] entry whose tag is present in the built tree.
    tap_targets: Vec<(Rect, &'static str)>,
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
        Self {
            bounds,
            inner: RefCell::new(None),
            visible: false,
            clear_countdown: 0,
        }
    }

    fn ensure_inner(&self) {
        if self.inner.borrow().is_some() {
            return;
        }
        *self.inner.borrow_mut() = Some(MediaPlayerInner::new(self.bounds));
    }

    /// Show or hide the skin. When hidden, `draw` is a no-op.
    pub fn set_visible(&mut self, v: bool) {
        if v {
            self.ensure_inner();
        } else if self.visible {
            self.clear_countdown = CLEAR_FRAMES;
        }
        self.visible = v;
    }

    /// Whether the skin is currently shown.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Step the machine with one event and re-apply the reactive bindings, so
    /// the bound artwork tracks the new machine state (QT-05g `is_active` swap,
    /// QT-05i chained swap, QT-05h hide/show). The machine owns every decision;
    /// the skin only forwards the event and refreshes.
    #[cfg(test)]
    fn step_event(&self, event: &str) {
        self.ensure_inner();
        let inner = self.inner.borrow();
        let Some(inner) = inner.as_ref() else {
            return;
        };
        inner.step_event(event);
    }

    /// Bounds of a tagged node in the emitted tree (for the pixel gate).
    #[cfg(test)]
    fn tagged_bounds(&self, tag: &str) -> Option<Rect> {
        self.ensure_inner();
        self.inner
            .borrow()
            .as_ref()
            .and_then(|inner| find_bounds_by_tag(&inner.node, tag))
    }

    /// Step the machine with a raw event and re-apply the bindings — used by
    /// the pixel gate to exercise display-only states (e.g. mute, shuffle) that
    /// have no tap target wired here. Same mechanism as a real tap.
    #[cfg(test)]
    fn test_step(&self, event: &str) {
        self.step_event(event);
    }

    /// Read the current text of the external-text Label bound to `key` (QT-05k),
    /// for the resolver gate. Walks the bindings for the `ExternalText` whose key
    /// matches and reads its Label handle directly.
    #[cfg(test)]
    fn external_text_for(&self, key: &str) -> Option<alloc::string::String> {
        use alloc::string::ToString;
        self.ensure_inner();
        self.inner.borrow().as_ref().and_then(|inner| {
            inner.bindings.iter().find_map(|b| match b {
                Binding::ExternalText(t) if t.key == key => {
                    Some(t.label.borrow().text().to_string())
                }
                _ => None,
            })
        })
    }
}

impl MediaPlayerInner {
    fn new(bounds: Rect) -> Self {
        // The emitted tree is self-positioning from `bounds` via the QT-03c
        // anchor solver. `build_screen` constructs + `start()`s the machine
        // (linkage v2) and returns the reactive bindings; the skin owns them.
        let (node, state, machine, bindings) = media_player_gen::build_screen(bounds);
        // QT-05j: resolve each emitted tap target (tag → raw QML event) to
        // (bounds, machine event) by joining against MEDIA_FUNC_MAP. Buttons the
        // map doesn't cover (e.g. MediaFunc.Scan) or tags absent from the tree
        // are dropped.
        let tap_targets: Vec<(Rect, &'static str)> = media_player_gen::BUTTON_TAP_EVENTS
            .iter()
            .filter_map(|(tag, qml_event)| {
                let bounds = find_bounds_by_tag(&node, tag)?;
                let machine_event = MEDIA_FUNC_MAP
                    .iter()
                    .find(|(q, _)| q == qml_event)
                    .map(|(_, ev)| *ev)?;
                Some((bounds, machine_event))
            })
            .collect();
        // `build_screen` constructs + `start()`s the machine, leaving it in
        // `mediaPlayerIdle`. Seed it past idle to the transport state
        // (`mediaStopped`) so the Play button is live — mirrors the
        // `MediaPlayerAdapter` config seeding (SCTD-03 §8). This is machine
        // initialization, not a predicate branch.
        {
            let mut m = machine.borrow_mut();
            m.step("Inp.Media.Ready", media_player::Value::Undefined);
            m.step("Inp.Media.ValidSource", media_player::Value::Undefined);
        }
        // Apply the initial machine-driven artwork (Play at rest — stopped).
        media_player_gen::refresh_bindings(&state, &machine, &bindings);
        // QT-05k: apply the consumer-owned resolver to the external-text
        // bindings (the source caption reads `audioPlayer.currentPlayUrlFileName`,
        // an external object — not machine state). A live consumer would re-apply
        // this whenever its player signals a track change; the demo's source is
        // fixed, so once at construction suffices.
        media_player_gen::apply_external_text(&bindings, resolve_external_text);
        Self {
            node,
            state,
            machine,
            bindings,
            tap_targets,
        }
    }

    fn step_event(&self, event: &str) {
        self.machine
            .borrow_mut()
            .step(event, media_player::Value::Undefined);
        media_player_gen::refresh_bindings(&self.state, &self.machine, &self.bindings);
    }
}

impl Widget for MediaPlayerSkin {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        if self.visible {
            self.ensure_inner();
            if let Some(inner) = self.inner.borrow().as_ref() {
                inner.node.draw(renderer);
            }
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if !self.visible {
            return false;
        }
        if let Event::PressRelease { x, y } | Event::PointerUp { x, y } = *event
            && (x < self.bounds.x
                || x >= self.bounds.x + self.bounds.width
                || y < self.bounds.y
                || y >= self.bounds.y + self.bounds.height)
        {
            return false;
        }
        self.ensure_inner();
        let mut inner = self.inner.borrow_mut();
        let Some(inner) = inner.as_mut() else {
            return false;
        };
        // A tap in any wired control region steps the machine (which swaps the
        // icon via the predicate / chain binding). `PressRelease` is the
        // committed tap trigger (see rlvgl CLAUDE.md runtime protocol).
        if let Event::PressRelease { x, y } | Event::PointerUp { x, y } = *event {
            for (b, ev) in &inner.tap_targets {
                if x >= b.x && x < b.x + b.width && y >= b.y && y < b.y + b.height {
                    inner.step_event(ev);
                    return true;
                }
            }
        }
        // Other taps still forward into the emitted tree (ClickArea regions).
        inner.node.dispatch_event(event)
    }

    fn clear_region(&mut self) -> Option<Rect> {
        if self.clear_countdown > 0 && !self.visible {
            self.clear_countdown -= 1;
            Some(self.bounds)
        } else {
            None
        }
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
    /// the colorful Bolero photo + icon pixels.
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
            .tagged_bounds("__rep_btn_1")
            .expect("Play button (__rep_btn_1) must be present in the emitted tree");

        // Frame 1: at rest (machine stopped → Play icon).
        let mut fb1 = Framebuffer::new(720, 480);
        skin.draw(&mut fb1);

        // Tap the center of the Play button → step(PlayPause) → playing.
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

    /// QT-05h reactive gate: the mute icon is hidden at rest (muteOff) and
    /// shown after the machine enters muteOn — so the icon's region changes
    /// when mute toggles, and the two hidden renders are identical (a complete
    /// hide, not a partial one). `imgMute` sits alone in the header strip, so
    /// the only thing changing in its region between renders is the icon.
    #[test]
    fn mute_icon_hidden_at_rest_shown_when_muted() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 720,
            height: 480,
        };
        let mut skin = MediaPlayerSkin::new(bounds);
        skin.set_visible(true);
        let mb = skin
            .tagged_bounds("imgMute")
            .expect("imgMute must be present in the emitted tree");

        // At rest: muteOff → imgMute hidden (region = background only).
        let mut fb_off = Framebuffer::new(720, 480);
        skin.draw(&mut fb_off);

        // Toggle mute → muteOn → imgMute shown: the region must change.
        skin.test_step("Inp.Media.Mute");
        let mut fb_on = Framebuffer::new(720, 480);
        skin.draw(&mut fb_on);
        assert!(
            region_diff(&fb_off, &fb_on, mb) > 0,
            "muting must show imgMute — the visibility binding is inert"
        );

        // Toggle back → muteOff → hidden again, identical to the first hidden
        // render (complete hide).
        skin.test_step("Inp.Media.Mute");
        let mut fb_off2 = Framebuffer::new(720, 480);
        skin.draw(&mut fb_off2);
        assert!(
            region_diff(&fb_on, &fb_off2, mb) > 0,
            "un-muting must hide imgMute again"
        );
        assert_eq!(
            region_diff(&fb_off, &fb_off2, mb),
            0,
            "the two muteOff renders must be identical in the imgMute region (complete hide)"
        );
    }

    /// QT-05i chained-predicate gate: tapping the repeat button cycles the
    /// machine through `mediaRepeatOff` → `Track` → `Folder` → `Off`, and the
    /// `Binding::Chain` swaps the repeat icon between NoRepeat / TrackRepeat /
    /// FolderRepeat artwork. Each of the three taps MUST change the repeatBtn
    /// region, and after three taps the artwork returns to the resting NoRepeat
    /// frame (identical to the start).
    #[test]
    fn repeat_button_artwork_cycles_on_tap() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 720,
            height: 480,
        };
        let mut skin = MediaPlayerSkin::new(bounds);
        skin.set_visible(true);
        let rb = skin
            .tagged_bounds("repeatBtn")
            .expect("repeatBtn must be present in the emitted tree");

        // Frame 0: at rest (mediaRepeatOff → NoRepeat icon).
        let mut fb0 = Framebuffer::new(720, 480);
        skin.draw(&mut fb0);

        let tap = |skin: &mut MediaPlayerSkin| {
            let consumed = skin.handle_event(&Event::PressRelease {
                x: rb.x + rb.width / 2,
                y: rb.y + rb.height / 2,
            });
            assert!(consumed, "tap in the repeatBtn region must be consumed");
        };

        // Tap 1: Off → Track. Tap 2: Track → Folder. Each must change the icon.
        tap(&mut skin);
        let mut fb1 = Framebuffer::new(720, 480);
        skin.draw(&mut fb1);
        assert!(
            region_diff(&fb0, &fb1, rb) > 0,
            "repeat Off→Track did not change the icon — the chain binding is inert"
        );

        tap(&mut skin);
        let mut fb2 = Framebuffer::new(720, 480);
        skin.draw(&mut fb2);
        assert!(
            region_diff(&fb1, &fb2, rb) > 0,
            "repeat Track→Folder did not change the icon — the chain binding is inert"
        );

        // Tap 3: Folder → Off. The icon must change again AND return to the
        // resting NoRepeat frame (chain default), proving the full cycle.
        tap(&mut skin);
        let mut fb3 = Framebuffer::new(720, 480);
        skin.draw(&mut fb3);
        assert!(
            region_diff(&fb2, &fb3, rb) > 0,
            "repeat Folder→Off did not change the icon — the chain binding is inert"
        );
        assert_eq!(
            region_diff(&fb0, &fb3, rb),
            0,
            "after a full Off→Track→Folder→Off cycle the repeat icon must match \
             the resting NoRepeat frame (chain default)"
        );
    }

    /// QT-05i / QT-05g binary gate: toggling shuffle (`Inp.Media.Shuffle`)
    /// enters `mediaPlayMixModeOn` and the `Binding::Predicate` swaps the
    /// shuffle icon Off→On. The shuffle button is untagged upstream, so it has
    /// no tap target; the only visual a bare Shuffle event can change is the
    /// shuffle icon, so a frame-level diff proves the binding is live, and
    /// toggling back returns the frame to the resting state.
    #[test]
    fn shuffle_icon_swaps_when_toggled() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 720,
            height: 480,
        };
        let full = bounds;
        let mut skin = MediaPlayerSkin::new(bounds);
        skin.set_visible(true);

        // At rest: mediaPlayMixModeOff → ShuffleOff icon.
        let mut fb_off = Framebuffer::new(720, 480);
        skin.draw(&mut fb_off);

        // Toggle shuffle on → ShuffleOn icon: the frame must change.
        skin.test_step("Inp.Media.Shuffle");
        let mut fb_on = Framebuffer::new(720, 480);
        skin.draw(&mut fb_on);
        assert!(
            region_diff(&fb_off, &fb_on, full) > 0,
            "toggling shuffle on must swap the shuffle icon — the predicate binding is inert"
        );

        // Toggle back off → resting frame again, identical to the start.
        skin.test_step("Inp.Media.Shuffle");
        let mut fb_off2 = Framebuffer::new(720, 480);
        skin.draw(&mut fb_off2);
        assert!(
            region_diff(&fb_on, &fb_off2, full) > 0,
            "toggling shuffle off must swap the icon back"
        );
        assert_eq!(
            region_diff(&fb_off, &fb_off2, full),
            0,
            "the two shuffle-off renders must be identical (clean toggle)"
        );
    }

    /// QT-05j end-to-end gate: a TAP on the shuffle button (now tap-wired via
    /// the emitted `BUTTON_TAP_EVENTS` + `MEDIA_FUNC_MAP`, where before it was
    /// inert) steps the machine into `mediaPlayMixModeOn` and the predicate
    /// binding swaps the shuffle icon. The button's synthetic tag is
    /// `__btn_mediafunc_shuffle` (no QML `id:` upstream).
    #[test]
    fn shuffle_button_artwork_swaps_on_tap() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 720,
            height: 480,
        };
        let mut skin = MediaPlayerSkin::new(bounds);
        skin.set_visible(true);
        let sb = skin
            .tagged_bounds("__btn_mediafunc_shuffle")
            .expect("shuffle button must carry the synthetic QT-05j tap tag");

        let mut fb1 = Framebuffer::new(720, 480);
        skin.draw(&mut fb1);

        let consumed = skin.handle_event(&Event::PressRelease {
            x: sb.x + sb.width / 2,
            y: sb.y + sb.height / 2,
        });
        assert!(
            consumed,
            "tap in the shuffle-button region must be consumed"
        );

        let mut fb2 = Framebuffer::new(720, 480);
        skin.draw(&mut fb2);
        assert!(
            region_diff(&fb1, &fb2, sb) > 0,
            "tapping shuffle must swap its icon — the QT-05j tap wiring is inert"
        );
    }

    /// QT-05k gate: the source-caption Label (`textSource`, whose QML `text:` is
    /// `audioPlayer.currentPlayUrlFileName` — an external object, not machine
    /// state) shows the value supplied by the consumer-owned resolver. This
    /// proves the full external-text round-trip: the emitter derived the key into
    /// a `Binding::ExternalText`, and `apply_external_text` wrote the resolver's
    /// value onto the Label. The emitter must NOT bake the value — it is
    /// app-owned (authority: derive).
    #[test]
    fn external_text_caption_resolves_from_consumer() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 720,
            height: 480,
        };
        let skin = MediaPlayerSkin::new(bounds);

        // The emitter surfaced the (tag, key) pair on the app-agnostic table.
        assert!(
            media_player_gen::EXTERNAL_TEXT_BINDINGS
                .iter()
                .any(|(tag, key)| *tag == "textSource"
                    && *key == "audioPlayer.currentPlayUrlFileName"),
            "EXTERNAL_TEXT_BINDINGS must surface the source-caption (tag, key)"
        );

        // The consumer resolver's value landed on the Label via apply_external_text.
        assert_eq!(
            skin.external_text_for("audioPlayer.currentPlayUrlFileName")
                .as_deref(),
            Some("Bolero - Ravel.mp3"),
            "the source caption must show the consumer-resolved external value"
        );
    }
}
