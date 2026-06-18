//! State-specific segmented image button (LPAR-12b).
//!
//! [`ImageButton`] presents one of six visual states — released, pressed,
//! disabled, checked-released, checked-pressed, checked-disabled — by selecting
//! from per-state left/middle/right [`ImageDescriptor`] sources.  The state
//! fallback chain mirrors LVGL's `lv_imagebutton` behavior.

use alloc::boxed::Box;
use rlvgl_core::event::Event;
use rlvgl_core::image::{BlitOpts, ImageDescriptor};
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Rect, Widget};

/// Visual state of an [`ImageButton`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageButtonState {
    /// Normal unpressed state.
    Released,
    /// Pointer currently held inside the button.
    Pressed,
    /// Button is disabled and cannot be activated.
    Disabled,
    /// Button is checked (toggled on) and not pressed.
    CheckedReleased,
    /// Button is checked and the pointer is currently held inside.
    CheckedPressed,
    /// Button is checked and disabled.
    CheckedDisabled,
}

/// Per-state left/middle/right image sources for an [`ImageButton`].
///
/// Sources use [`ImageDescriptor<'static>`] so the widget can be fully owned.
/// Supply [`rlvgl_core::image::ImageData::Owned`] pixel data for heap-resident
/// sources, or [`rlvgl_core::image::ImageData::Borrowed`] with a `'static`
/// pixel slice for ROM-resident sources.
#[derive(Default)]
pub struct ImageButtonSources {
    /// Optional left-edge image segment.
    pub left: Option<Box<ImageDescriptor<'static>>>,
    /// Optional center / fill image segment.  Required for segmented drawing.
    pub middle: Option<Box<ImageDescriptor<'static>>>,
    /// Optional right-edge image segment.
    pub right: Option<Box<ImageDescriptor<'static>>>,
}

fn state_index(s: ImageButtonState) -> usize {
    match s {
        ImageButtonState::Released => 0,
        ImageButtonState::Pressed => 1,
        ImageButtonState::Disabled => 2,
        ImageButtonState::CheckedReleased => 3,
        ImageButtonState::CheckedPressed => 4,
        ImageButtonState::CheckedDisabled => 5,
    }
}

/// State-specific segmented image button.
///
/// # State fallback (§5.D)
///
/// When a state's middle source is absent, the widget walks a fallback chain
/// until a middle source is found:
///
/// * `Pressed` → `Released`
/// * `CheckedReleased` → `Released`
/// * `CheckedPressed` → `CheckedReleased` → `Pressed` → `Released`
/// * `Disabled` → `Released`
/// * `CheckedDisabled` → `CheckedReleased` → `Released`
///
/// # Event handling
///
/// Pointer `PressDown` inside bounds sets the visual pressed state.
/// `PressRelease` inside bounds activates the button (returns `true`).
/// Disabled buttons consume no activation.
pub struct ImageButton {
    bounds: Rect,
    /// App-controlled base state (disabled, checked variants).
    state: ImageButtonState,
    /// Whether the button is in the checked (toggled on) state.
    checked: bool,
    /// Whether a pointer is currently held inside bounds.
    is_pressed: bool,
    /// Per-state image sources, indexed by [`state_index`].
    sources: [ImageButtonSources; 6],
}

impl ImageButton {
    /// Create a new image button occupying `bounds` in the released state.
    pub fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            state: ImageButtonState::Released,
            checked: false,
            is_pressed: false,
            sources: [
                ImageButtonSources::default(),
                ImageButtonSources::default(),
                ImageButtonSources::default(),
                ImageButtonSources::default(),
                ImageButtonSources::default(),
                ImageButtonSources::default(),
            ],
        }
    }

    /// Set the left, middle, and right sources for `state`.
    ///
    /// Pass `None` for any segment that should be absent.  The middle segment
    /// must be set for the button to draw content; a missing middle is allowed
    /// but produces no visible image.
    pub fn set_src(
        &mut self,
        state: ImageButtonState,
        left: Option<ImageDescriptor<'static>>,
        middle: Option<ImageDescriptor<'static>>,
        right: Option<ImageDescriptor<'static>>,
    ) {
        let slot = &mut self.sources[state_index(state)];
        slot.left = left.map(Box::new);
        slot.middle = middle.map(Box::new);
        slot.right = right.map(Box::new);
    }

    /// Return the left source configured for `state` (no fallback applied).
    pub fn src_left(&self, state: ImageButtonState) -> Option<&ImageDescriptor<'static>> {
        self.sources[state_index(state)].left.as_deref()
    }

    /// Return the middle source configured for `state` (no fallback applied).
    pub fn src_middle(&self, state: ImageButtonState) -> Option<&ImageDescriptor<'static>> {
        self.sources[state_index(state)].middle.as_deref()
    }

    /// Return the right source configured for `state` (no fallback applied).
    pub fn src_right(&self, state: ImageButtonState) -> Option<&ImageDescriptor<'static>> {
        self.sources[state_index(state)].right.as_deref()
    }

    /// Set the app-controlled base state.
    ///
    /// Use [`ImageButtonState::Disabled`] or [`ImageButtonState::CheckedDisabled`]
    /// to prevent pointer activation.
    pub fn set_state(&mut self, state: ImageButtonState) {
        self.state = state;
    }

    /// Return the app-controlled base state.
    pub fn state(&self) -> ImageButtonState {
        self.state
    }

    /// Set the checked (toggled on) state.
    pub fn set_checked(&mut self, checked: bool) {
        self.checked = checked;
    }

    /// Return `true` when the button is in the checked state.
    pub fn checked(&self) -> bool {
        self.checked
    }

    /// Compute the effective draw state from the base state, checked flag, and
    /// whether a pointer is currently held.
    pub fn draw_state(&self) -> ImageButtonState {
        if matches!(
            self.state,
            ImageButtonState::Disabled | ImageButtonState::CheckedDisabled
        ) {
            if self.checked {
                return ImageButtonState::CheckedDisabled;
            }
            return ImageButtonState::Disabled;
        }
        match (self.checked, self.is_pressed) {
            (true, true) => ImageButtonState::CheckedPressed,
            (true, false) => ImageButtonState::CheckedReleased,
            (false, true) => ImageButtonState::Pressed,
            (false, false) => ImageButtonState::Released,
        }
    }

    /// Walk the fallback chain and return the state whose middle source exists.
    ///
    /// Returns the terminal fallback state even when its middle is absent.
    fn resolved_state(&self) -> ImageButtonState {
        let chain: &[ImageButtonState] = match self.draw_state() {
            ImageButtonState::Released => &[ImageButtonState::Released],
            ImageButtonState::Pressed => &[ImageButtonState::Pressed, ImageButtonState::Released],
            ImageButtonState::Disabled => &[ImageButtonState::Disabled, ImageButtonState::Released],
            ImageButtonState::CheckedReleased => &[
                ImageButtonState::CheckedReleased,
                ImageButtonState::Released,
            ],
            ImageButtonState::CheckedPressed => &[
                ImageButtonState::CheckedPressed,
                ImageButtonState::CheckedReleased,
                ImageButtonState::Pressed,
                ImageButtonState::Released,
            ],
            ImageButtonState::CheckedDisabled => &[
                ImageButtonState::CheckedDisabled,
                ImageButtonState::CheckedReleased,
                ImageButtonState::Released,
            ],
        };
        for &s in chain {
            if self.sources[state_index(s)].middle.is_some() {
                return s;
            }
        }
        // Terminal: return last in chain even if middle is absent
        *chain.last().unwrap_or(&ImageButtonState::Released)
    }

    fn inside_bounds(&self, x: i32, y: i32) -> bool {
        let b = self.bounds;
        x >= b.x && x < b.x + b.width && y >= b.y && y < b.y + b.height
    }

    fn is_disabled(&self) -> bool {
        matches!(
            self.state,
            ImageButtonState::Disabled | ImageButtonState::CheckedDisabled
        )
    }
}

impl Widget for ImageButton {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        let s = self.resolved_state();
        let slot = &self.sources[state_index(s)];
        let opts = BlitOpts::default();

        match (&slot.left, &slot.middle, &slot.right) {
            (Some(left), Some(middle), Some(right)) => {
                let lw = left.width as i32;
                let rw = right.width as i32;
                let mid_w = (self.bounds.width - lw - rw).max(0);

                let left_rect = Rect {
                    x: self.bounds.x,
                    y: self.bounds.y,
                    width: lw,
                    height: self.bounds.height,
                };
                renderer.blit_image(left_rect, left, &opts);

                if mid_w > 0 {
                    let mid_rect = Rect {
                        x: self.bounds.x + lw,
                        y: self.bounds.y,
                        width: mid_w,
                        height: self.bounds.height,
                    };
                    renderer.blit_image(mid_rect, middle, &opts);
                }

                let right_rect = Rect {
                    x: self.bounds.x + self.bounds.width - rw,
                    y: self.bounds.y,
                    width: rw,
                    height: self.bounds.height,
                };
                renderer.blit_image(right_rect, right, &opts);
            }
            (_, Some(middle), _) => {
                // Middle-only (or partial): fill the full bounds with middle
                renderer.blit_image(self.bounds, middle, &opts);
            }
            _ => {
                // No sources configured — nothing to draw
            }
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::PressDown { x, y } if !self.is_disabled() && self.inside_bounds(*x, *y) => {
                self.is_pressed = true;
                true
            }
            Event::PressRelease { x, y }
                if !self.is_disabled() && self.is_pressed && self.inside_bounds(*x, *y) =>
            {
                self.is_pressed = false;
                true // activation consumed; app registers callbacks via object handlers
            }
            Event::PointerUp { .. } => {
                self.is_pressed = false;
                false
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rlvgl_core::image::PixelFormat;
    use rlvgl_core::widget::Color;

    fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    fn empty_descriptor() -> ImageDescriptor<'static> {
        ImageDescriptor::owned(PixelFormat::Argb8888, 4, 4, alloc::vec![0u8; 64])
    }

    struct NullRenderer;
    impl rlvgl_core::renderer::Renderer for NullRenderer {
        fn fill_rect(&mut self, _rect: Rect, _color: Color) {}
        fn draw_text(&mut self, _pos: (i32, i32), _text: &str, _color: Color) {}
    }

    #[test]
    fn new_starts_released_unchecked() {
        let btn = ImageButton::new(rect(0, 0, 100, 30));
        assert_eq!(btn.state(), ImageButtonState::Released);
        assert!(!btn.checked());
    }

    #[test]
    fn set_src_and_retrieve_without_fallback() {
        let mut btn = ImageButton::new(rect(0, 0, 100, 30));
        btn.set_src(
            ImageButtonState::Released,
            None,
            Some(empty_descriptor()),
            None,
        );
        assert!(btn.src_middle(ImageButtonState::Released).is_some());
        assert!(btn.src_left(ImageButtonState::Released).is_none());
    }

    #[test]
    fn pressed_event_sets_is_pressed() {
        let mut btn = ImageButton::new(rect(0, 0, 100, 30));
        let consumed = btn.handle_event(&Event::PressDown { x: 50, y: 15 });
        assert!(consumed);
        assert_eq!(btn.draw_state(), ImageButtonState::Pressed);
    }

    #[test]
    fn press_release_inside_activates() {
        let mut btn = ImageButton::new(rect(0, 0, 100, 30));
        btn.handle_event(&Event::PressDown { x: 50, y: 15 });
        let activated = btn.handle_event(&Event::PressRelease { x: 50, y: 15 });
        assert!(activated);
        assert_eq!(btn.draw_state(), ImageButtonState::Released);
    }

    #[test]
    fn press_outside_bounds_not_consumed() {
        let mut btn = ImageButton::new(rect(0, 0, 100, 30));
        let consumed = btn.handle_event(&Event::PressDown { x: 200, y: 15 });
        assert!(!consumed);
    }

    #[test]
    fn disabled_state_ignores_pointer() {
        let mut btn = ImageButton::new(rect(0, 0, 100, 30));
        btn.set_state(ImageButtonState::Disabled);
        let consumed = btn.handle_event(&Event::PressDown { x: 50, y: 15 });
        assert!(!consumed);
    }

    #[test]
    fn checked_state_produces_checked_released() {
        let mut btn = ImageButton::new(rect(0, 0, 100, 30));
        btn.set_checked(true);
        assert_eq!(btn.draw_state(), ImageButtonState::CheckedReleased);
    }

    #[test]
    fn checked_and_pressed_produces_checked_pressed() {
        let mut btn = ImageButton::new(rect(0, 0, 100, 30));
        btn.set_checked(true);
        btn.handle_event(&Event::PressDown { x: 50, y: 15 });
        assert_eq!(btn.draw_state(), ImageButtonState::CheckedPressed);
    }

    #[test]
    fn fallback_checked_released_to_released() {
        let mut btn = ImageButton::new(rect(0, 0, 100, 30));
        btn.set_src(
            ImageButtonState::Released,
            None,
            Some(empty_descriptor()),
            None,
        );
        // CheckedReleased has no source; should fall back to Released
        btn.set_checked(true);
        assert_eq!(btn.resolved_state(), ImageButtonState::Released);
    }

    #[test]
    fn draw_does_not_panic_with_no_sources() {
        let btn = ImageButton::new(rect(0, 0, 100, 30));
        let mut r = NullRenderer;
        btn.draw(&mut r);
    }

    #[test]
    fn set_bounds_adopted() {
        let mut btn = ImageButton::new(rect(0, 0, 100, 30));
        btn.set_bounds(rect(10, 20, 200, 50));
        assert_eq!(btn.bounds(), rect(10, 20, 200, 50));
    }

    #[test]
    fn pointer_up_clears_pressed_returns_false() {
        let mut btn = ImageButton::new(rect(0, 0, 100, 30));
        btn.handle_event(&Event::PressDown { x: 50, y: 15 });
        let r = btn.handle_event(&Event::PointerUp { x: 50, y: 15 });
        assert!(!r);
        assert_eq!(btn.draw_state(), ImageButtonState::Released);
    }
}
