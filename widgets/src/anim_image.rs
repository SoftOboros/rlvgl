//! Tick-driven animated image widget.
//!
//! [`AnimImage`] displays a sequence of [`ImageDescriptor`] frames, advancing
//! them deterministically on [`Event::Tick`] using a local integer frame phase
//! — exactly the same pattern as [`crate::spinner::Spinner`] (`phase_tick`
//! incremented on each `Tick`, frame index derived without wall-clock time).
//!
//! ## Frame source
//!
//! Frames are supplied by the caller as a [`FrameSource`]: either a
//! heap-resident `Vec<ImageDescriptor<'static>>` obtained by decoding a GIF or
//! APNG through the `core::plugins::{gif,apng}` decode functions (std-only,
//! done once at load time outside this widget), or a borrow of a static frame
//! array for embedded targets.  The widget never decodes image data internally.
//!
//! ## Tick model
//!
//! Each `Event::Tick` increments an internal `frame_tick` counter.  Every
//! `ticks_per_frame` ticks (clamped to ≥ 1) the displayed frame index advances.
//! When `loop_mode` is [`AnimImageLoopMode::Once`], the widget stops at the
//! last frame and fires the `on_complete` callback.  [`AnimImageLoopMode::Bounce`]
//! reverses direction at each end.  `set_reverse(true)` permanently reverses
//! the initial direction.
//!
//! ## Drawing
//!
//! `Part::MAIN` — widget background.
//! `Part::INDICATOR` — current frame blitted into `bounds` via
//! [`Renderer::blit_image`].

use alloc::boxed::Box;
use alloc::vec::Vec;

use rlvgl_core::draw::draw_widget_bg;
use rlvgl_core::event::Event;
use rlvgl_core::image::{BlitOpts, ImageDescriptor};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Rect, Widget};

// ──────────────────────────────────────────────────────────────────────────
// Public enumerations
// ──────────────────────────────────────────────────────────────────────────

/// Source of pre-decoded [`ImageDescriptor`] frames for [`AnimImage`].
///
/// The widget never decodes image data; frames are provided by the caller.
/// For `std` targets, decode GIF/APNG once using `rlvgl_core::plugins::gif` /
/// `rlvgl_core::plugins::apng` and convert each frame to an
/// `ImageDescriptor<'static>` before constructing [`AnimImage`].
pub enum FrameSource {
    /// Heap-resident frames decoded at load time.
    Decoded(Vec<ImageDescriptor<'static>>),
    /// Statically allocated frame array for `no_std` / ROM targets.
    Static(&'static [ImageDescriptor<'static>]),
}

impl FrameSource {
    /// Return the number of frames available.
    pub fn frame_count(&self) -> usize {
        match self {
            FrameSource::Decoded(v) => v.len(),
            FrameSource::Static(s) => s.len(),
        }
    }

    /// Return the frame at `index`, or `None` when out of bounds.
    pub fn get(&self, index: usize) -> Option<&ImageDescriptor<'static>> {
        match self {
            FrameSource::Decoded(v) => v.get(index),
            FrameSource::Static(s) => s.get(index),
        }
    }
}

/// Playback state for [`AnimImage`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimPlayState {
    /// Animation is playing; `Event::Tick` advances the frame counter.
    Running,
    /// Animation is paused; `Event::Tick` is ignored.
    Paused,
}

/// Frame-advance loop mode for [`AnimImage`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimImageLoopMode {
    /// Wrap back to frame 0 after the last frame (infinite loop).
    Loop,
    /// Play once and stop at the last frame; fires `on_complete`.
    Once,
    /// Ping-pong: reverse direction at each end.
    Bounce,
}

// ──────────────────────────────────────────────────────────────────────────
// AnimImage
// ──────────────────────────────────────────────────────────────────────────

/// Tick-driven animated image widget.
///
/// See the [module-level documentation](self) for the full design contract.
pub struct AnimImage {
    bounds: Rect,
    /// Visual style for the widget background (`Part::MAIN`).
    pub style: Style,
    frames: FrameSource,
    /// Number of `Event::Tick`s that must accumulate before the frame index
    /// advances.  Always `≥ 1` (zero is clamped on write).
    ticks_per_frame: u32,
    /// Local tick counter, reset each full frame period.
    frame_tick: u32,
    /// Current displayed frame index.
    frame_index: usize,
    play_state: AnimPlayState,
    loop_mode: AnimImageLoopMode,
    /// When `true`, frames advance in decreasing index order.
    reverse: bool,
    /// Bounce direction: `true` means currently advancing forward.
    bounce_forward: bool,
    /// Optional callback fired when `AnimImageLoopMode::Once` reaches the
    /// last frame.
    on_complete: Option<Box<dyn FnMut()>>,
}

impl AnimImage {
    /// Create an animated image at `bounds` from the provided `frames`.
    ///
    /// Default: `Running`, `Loop`, 3 ticks/frame, forward direction.
    pub fn new(bounds: Rect, frames: FrameSource) -> Self {
        Self {
            bounds,
            style: Style::default(),
            frames,
            ticks_per_frame: 3,
            frame_tick: 0,
            frame_index: 0,
            play_state: AnimPlayState::Running,
            loop_mode: AnimImageLoopMode::Loop,
            reverse: false,
            bounce_forward: true,
            on_complete: None,
        }
    }

    // ── playback speed ─────────────────────────────────────────────────────

    /// Set the number of ticks between frame advances.  `0` is clamped to `1`.
    pub fn set_ticks_per_frame(&mut self, n: u32) {
        self.ticks_per_frame = n.max(1);
        // Keep frame_tick in range.
        self.frame_tick = self.frame_tick.min(self.ticks_per_frame - 1);
    }

    /// Number of ticks between frame advances (always ≥ 1).
    pub fn ticks_per_frame(&self) -> u32 {
        self.ticks_per_frame
    }

    // ── frame access ───────────────────────────────────────────────────────

    /// Number of frames in the animation (may be zero for an empty source).
    pub fn frame_count(&self) -> usize {
        self.frames.frame_count()
    }

    /// Index of the currently displayed frame.
    pub fn current_frame_index(&self) -> usize {
        self.frame_index
    }

    /// Force-set the displayed frame index.  Out-of-bounds values are clamped
    /// to `frame_count().saturating_sub(1)`.
    pub fn set_current_frame(&mut self, index: usize) {
        let count = self.frames.frame_count();
        self.frame_index = if count == 0 { 0 } else { index.min(count - 1) };
    }

    // ── play / pause ───────────────────────────────────────────────────────

    /// Start playback (equivalent to LVGL `lv_animimg_start`).
    pub fn play(&mut self) {
        self.play_state = AnimPlayState::Running;
    }

    /// Pause playback (equivalent to LVGL `lv_animimg_stop`).
    pub fn pause(&mut self) {
        self.play_state = AnimPlayState::Paused;
    }

    /// Set the play state directly.
    pub fn set_play_state(&mut self, state: AnimPlayState) {
        self.play_state = state;
    }

    /// Current play state.
    pub fn play_state(&self) -> AnimPlayState {
        self.play_state
    }

    /// Return `true` when the animation is currently running.
    pub fn is_playing(&self) -> bool {
        self.play_state == AnimPlayState::Running
    }

    // ── loop mode ──────────────────────────────────────────────────────────

    /// Set the loop mode.
    pub fn set_loop_mode(&mut self, mode: AnimImageLoopMode) {
        self.loop_mode = mode;
    }

    /// Current loop mode.
    pub fn loop_mode(&self) -> AnimImageLoopMode {
        self.loop_mode
    }

    // ── direction ──────────────────────────────────────────────────────────

    /// When `true`, the initial playback direction is reversed (frame index
    /// decrements toward 0 rather than incrementing).
    pub fn set_reverse(&mut self, reverse: bool) {
        self.reverse = reverse;
        // Keep bounce state in sync with the new preferred direction.
        self.bounce_forward = !reverse;
    }

    // ── completion callback ────────────────────────────────────────────────

    /// Register a callback fired when `AnimImageLoopMode::Once` reaches the
    /// last frame.  Replaces any previously registered callback.
    pub fn on_complete<F: FnMut() + 'static>(mut self, handler: F) -> Self {
        self.on_complete = Some(Box::new(handler));
        self
    }

    // ── internal advance ──────────────────────────────────────────────────

    /// Advance the frame counter by one tick.  Returns `true` when the frame
    /// index actually changed so the caller can schedule a repaint.
    fn advance_tick(&mut self) -> bool {
        let count = self.frames.frame_count();
        if count == 0 {
            return false;
        }

        self.frame_tick += 1;
        if self.frame_tick < self.ticks_per_frame {
            return false; // Not yet time for the next frame.
        }
        self.frame_tick = 0;

        let prev = self.frame_index;
        // Determine the effective advance direction for this tick.
        let forward = if self.loop_mode == AnimImageLoopMode::Bounce {
            self.bounce_forward
        } else {
            !self.reverse
        };

        if forward {
            if self.frame_index + 1 < count {
                self.frame_index += 1;
            } else {
                // Reached the last frame.
                match self.loop_mode {
                    AnimImageLoopMode::Loop => {
                        self.frame_index = 0;
                    }
                    AnimImageLoopMode::Once => {
                        // Stay on the last frame; fire completion callback.
                        self.play_state = AnimPlayState::Paused;
                        if let Some(cb) = &mut self.on_complete {
                            cb();
                        }
                    }
                    AnimImageLoopMode::Bounce => {
                        self.bounce_forward = false;
                        if count >= 2 {
                            self.frame_index = count - 2;
                        }
                    }
                }
            }
        } else if self.frame_index > 0 {
            self.frame_index -= 1;
        } else {
            // Reached frame 0 going backward.
            match self.loop_mode {
                AnimImageLoopMode::Loop => {
                    self.frame_index = count - 1;
                }
                AnimImageLoopMode::Once => {
                    self.play_state = AnimPlayState::Paused;
                    if let Some(cb) = &mut self.on_complete {
                        cb();
                    }
                }
                AnimImageLoopMode::Bounce => {
                    self.bounce_forward = true;
                    if count >= 2 {
                        self.frame_index = 1;
                    }
                }
            }
        }

        self.frame_index != prev
    }
}

impl Widget for AnimImage {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    /// Draw the current animation frame.
    ///
    /// `Part::MAIN` — widget background.
    /// `Part::INDICATOR` — current frame via [`Renderer::blit_image`].
    fn draw(&self, renderer: &mut dyn Renderer) {
        draw_widget_bg(renderer, self.bounds, &self.style);
        if let Some(frame) = self.frames.get(self.frame_index) {
            renderer.blit_image(self.bounds, frame, &BlitOpts::default());
        }
    }

    /// Handle events.  Returns `true` when [`Event::Tick`] is consumed
    /// (matching the Spinner pattern: Tick is always consumed when `Running`).
    fn handle_event(&mut self, event: &Event) -> bool {
        if matches!(event, Event::Tick) && self.play_state == AnimPlayState::Running {
            self.advance_tick();
            return true;
        }
        false
    }

    /// Adopt a layout-computed bounds rect.
    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rlvgl_core::image::{ImageData, PixelFormat};
    use rlvgl_core::widget::Color;

    // ── helpers ────────────────────────────────────────────────────────────

    fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    /// Build a trivially small static frame list (colors are dummies).
    fn make_frames(n: usize) -> FrameSource {
        // We need 'static descriptors — use Borrowed(&[]) as empty pixel data.
        let mut frames: Vec<ImageDescriptor<'static>> = Vec::new();
        for _ in 0..n {
            frames.push(ImageDescriptor::new(
                PixelFormat::Argb8888,
                1,
                1,
                ImageData::Borrowed(&[]),
                None,
            ));
        }
        FrameSource::Decoded(frames)
    }

    fn tick(anim: &mut AnimImage) -> bool {
        anim.handle_event(&Event::Tick)
    }

    // ── tick cadence ───────────────────────────────────────────────────────

    #[test]
    fn tick_advances_frame_at_right_cadence() {
        let mut a = AnimImage::new(rect(0, 0, 10, 10), make_frames(4));
        a.set_ticks_per_frame(3);
        // First two ticks should NOT advance the frame.
        assert!(tick(&mut a)); // consumed, no frame change
        assert_eq!(a.current_frame_index(), 0);
        assert!(tick(&mut a));
        assert_eq!(a.current_frame_index(), 0);
        // Third tick crosses the period boundary.
        assert!(tick(&mut a));
        assert_eq!(a.current_frame_index(), 1);
    }

    #[test]
    fn zero_period_is_clamped_to_one() {
        let mut a = AnimImage::new(rect(0, 0, 10, 10), make_frames(3));
        a.set_ticks_per_frame(0);
        assert_eq!(a.ticks_per_frame(), 1);
        tick(&mut a);
        assert_eq!(a.current_frame_index(), 1, "advances every tick");
    }

    // ── loop wrap ──────────────────────────────────────────────────────────

    #[test]
    fn loop_mode_wraps_at_last_frame() {
        let mut a = AnimImage::new(rect(0, 0, 10, 10), make_frames(3));
        a.set_ticks_per_frame(1);
        a.set_loop_mode(AnimImageLoopMode::Loop);
        tick(&mut a); // → 1
        tick(&mut a); // → 2
        tick(&mut a); // → wraps to 0
        assert_eq!(a.current_frame_index(), 0);
        assert!(a.is_playing());
    }

    // ── once mode ─────────────────────────────────────────────────────────

    #[test]
    fn once_mode_stops_at_last_frame() {
        let mut a = AnimImage::new(rect(0, 0, 10, 10), make_frames(3));
        a.set_ticks_per_frame(1);
        a.set_loop_mode(AnimImageLoopMode::Once);
        tick(&mut a); // → 1
        tick(&mut a); // → 2 (last)
        tick(&mut a); // stays at 2, pauses
        assert_eq!(a.current_frame_index(), 2);
        assert_eq!(a.play_state(), AnimPlayState::Paused);
    }

    #[test]
    fn once_mode_fires_on_complete() {
        use alloc::rc::Rc;
        use core::cell::Cell;
        let fired = Rc::new(Cell::new(0u32));
        let fired_clone = fired.clone();
        // 3-frame animation: indices 0, 1, 2.
        // Tick 1: 0→1, Tick 2: 1→2, Tick 3: would go past 2 → fires on_complete.
        let mut a = AnimImage::new(rect(0, 0, 10, 10), make_frames(3))
            .on_complete(move || fired_clone.set(fired_clone.get() + 1));
        a.set_ticks_per_frame(1);
        a.set_loop_mode(AnimImageLoopMode::Once);
        tick(&mut a); // 0→1
        tick(&mut a); // 1→2 (last frame)
        assert_eq!(a.current_frame_index(), 2);
        assert_eq!(
            fired.get(),
            0,
            "on_complete not fired until attempt to advance past last"
        );
        tick(&mut a); // attempt to advance past last → fires on_complete, stays at 2
        assert_eq!(fired.get(), 1, "on_complete fired exactly once");
        assert_eq!(a.play_state(), AnimPlayState::Paused);
    }

    // ── bounce mode ────────────────────────────────────────────────────────

    #[test]
    fn bounce_mode_reverses_at_endpoints() {
        let mut a = AnimImage::new(rect(0, 0, 10, 10), make_frames(3));
        a.set_ticks_per_frame(1);
        a.set_loop_mode(AnimImageLoopMode::Bounce);
        // Forward: 0 → 1 → 2 → reverses → 1 → 0 → reverses → 1
        let expected = [1, 2, 1, 0, 1];
        for &exp in &expected {
            tick(&mut a);
            assert_eq!(a.current_frame_index(), exp, "expected frame {exp}");
        }
    }

    // ── play / pause ───────────────────────────────────────────────────────

    #[test]
    fn paused_does_not_consume_tick() {
        let mut a = AnimImage::new(rect(0, 0, 10, 10), make_frames(4));
        a.pause();
        let consumed = tick(&mut a);
        assert!(!consumed, "Tick not consumed when Paused");
        assert_eq!(a.current_frame_index(), 0, "frame unchanged while paused");
    }

    #[test]
    fn play_resume_from_paused() {
        let mut a = AnimImage::new(rect(0, 0, 10, 10), make_frames(4));
        a.set_ticks_per_frame(1);
        a.pause();
        tick(&mut a);
        assert_eq!(a.current_frame_index(), 0);
        a.play();
        tick(&mut a);
        assert_eq!(a.current_frame_index(), 1);
    }

    // ── reverse direction ─────────────────────────────────────────────────

    #[test]
    fn reverse_starts_at_last_frame_going_backward() {
        let mut a = AnimImage::new(rect(0, 0, 10, 10), make_frames(3));
        a.set_ticks_per_frame(1);
        a.set_reverse(true);
        // Initial frame is 0; first tick moves backward → wraps in Loop → last
        tick(&mut a);
        assert_eq!(a.current_frame_index(), 2, "loop backward wraps to last");
    }

    // ── set_bounds ────────────────────────────────────────────────────────

    #[test]
    fn set_bounds_repositions() {
        let mut a = AnimImage::new(rect(0, 0, 50, 50), make_frames(2));
        a.set_bounds(rect(5, 10, 100, 80));
        assert_eq!(a.bounds(), rect(5, 10, 100, 80));
    }

    // ── set_current_frame ─────────────────────────────────────────────────

    #[test]
    fn set_current_frame_clamps_to_valid_range() {
        let mut a = AnimImage::new(rect(0, 0, 10, 10), make_frames(4));
        a.set_current_frame(100);
        assert_eq!(a.current_frame_index(), 3);
        a.set_current_frame(0);
        assert_eq!(a.current_frame_index(), 0);
    }

    // ── draw dispatches blit_image ─────────────────────────────────────────

    #[test]
    fn draw_calls_blit_image_for_current_frame() {
        struct BlitCatcher(bool);
        impl Renderer for BlitCatcher {
            fn fill_rect(&mut self, _r: Rect, _c: Color) {}
            fn draw_text(&mut self, _p: (i32, i32), _t: &str, _c: Color) {}
            fn blit_image(&mut self, _dest: Rect, _desc: &ImageDescriptor<'_>, _opts: &BlitOpts) {
                self.0 = true;
            }
        }
        let a = AnimImage::new(rect(0, 0, 10, 10), make_frames(1));
        let mut r = BlitCatcher(false);
        a.draw(&mut r);
        assert!(
            r.0,
            "blit_image should be called for non-empty frame source"
        );
    }
}
