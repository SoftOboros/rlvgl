//! Specialized [`ButtonMatrix`] with predefined key maps and key-output hook
//! (LPAR-13 §5.D).
//!
//! A [`Keyboard`] owns a [`ButtonMatrix`] internally and switches between
//! predefined button maps via [`set_mode`](Keyboard::set_mode).  Pressing a
//! key resolves to a [`KeyOutput`] value delivered to the caller via:
//!
//! 1. **Hook** — install a `FnMut(KeyOutput) + 'static` closure with
//!    [`set_key_output_hook`](Keyboard::set_key_output_hook).
//! 2. **Poll slot** — call [`last_key_output`](Keyboard::last_key_output) once
//!    per frame; it drains and returns the most-recently-produced output.
//!
//! Before LPAR-14 Textarea v2 the app wires the hook to any text field
//! manually (e.g. `textarea.insert_char(ch)`).
//!
//! # Key navigation (LPAR-12/LPAR-13 §5.J)
//!
//! Wire `ObjectEvent::Key` to the imperative helpers:
//!
//! ```ignore
//! keyboard.navigate_next();        // ArrowRight / Tab
//! keyboard.navigate_prev();        // ArrowLeft  / BackTab
//! keyboard.activate_selected();    // Enter
//! ```

use alloc::boxed::Box;
use alloc::string::String;
use alloc::string::ToString;
use rlvgl_core::event::Event;
use rlvgl_core::renderer::Renderer;
use rlvgl_core::widget::{Rect, Widget};

use crate::button_matrix::{BUTTON_NONE, ButtonId, ButtonMatrix};

// ── Predefined key maps ───────────────────────────────────────────────────────

/// Lowercase alphabetic map (mirrors `lv_kb_def_btnm_map_lc`).
static MAP_LOWER: &[&str] = &[
    "q", "w", "e", "r", "t", "y", "u", "i", "o", "p", "\n", "a", "s", "d", "f", "g", "h", "j", "k",
    "l", "\n", "ABC", "z", "x", "c", "v", "b", "n", "m", "⌫", "\n", "123", " ", "↵",
];

/// Uppercase alphabetic map (mirrors `lv_kb_def_btnm_map_uc`).
static MAP_UPPER: &[&str] = &[
    "Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P", "\n", "A", "S", "D", "F", "G", "H", "J", "K",
    "L", "\n", "abc", "Z", "X", "C", "V", "B", "N", "M", "⌫", "\n", "123", " ", "↵",
];

/// Numeric / symbol map (mirrors `lv_kb_def_btnm_map_num`).
static MAP_NUMBER: &[&str] = &[
    "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "\n", "+", "-", "/", "*", "=", "%", "!", "?",
    "#", "<", ">", "\n", "\\", "@", "$", "(", ")", "{", "}", "[", "]", ";", ":", "'", "\n", "abc",
    " ", "↵", "⌫",
];

/// Special / emoji placeholder map.
static MAP_SPECIAL: &[&str] = &[
    ":)", ":(", ";)", ":D", ":O", ":-|", ":P", "\n", "abc", " ", "↵", "⌫",
];

// ── Public types ──────────────────────────────────────────────────────────────

/// Named key-map configuration for [`Keyboard`].
///
/// Registration policy: Specification Required (LPAR-13 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardMode {
    /// Lowercase alphabetic layout.
    TextLower,
    /// Uppercase alphabetic layout.
    TextUpper,
    /// Numeric and symbol layout.
    Number,
    /// Special characters / emoji placeholder layout.
    Special,
    /// User-defined slot 1.
    User1,
    /// User-defined slot 2.
    User2,
    /// User-defined slot 3.
    User3,
    /// User-defined slot 4.
    User4,
}

/// Internal control action produced by a special key in the keyboard map.
///
/// Registration policy: Specification Required (LPAR-13 §7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyboardControl {
    /// Switch to the indicated keyboard mode.
    SwitchMode(KeyboardMode),
}

/// Value produced by a key activation on a [`Keyboard`].
///
/// Registration policy: Specification Required (LPAR-13 §7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyOutput {
    /// A printable character was pressed.
    Char(char),
    /// The backspace key was pressed (delete character before cursor).
    Backspace,
    /// The enter / return key was pressed.
    Enter,
    /// The tab key was pressed.
    Tab,
    /// The escape key was pressed.
    Escape,
    /// An internal control action (mode switch, etc.).
    Control(KeyboardControl),
}

/// Resolve a button label string from a predefined map to a [`KeyOutput`].
fn resolve_key(label: &str) -> Option<KeyOutput> {
    match label {
        "⌫" => Some(KeyOutput::Backspace),
        "↵" => Some(KeyOutput::Enter),
        "ABC" => Some(KeyOutput::Control(KeyboardControl::SwitchMode(
            KeyboardMode::TextUpper,
        ))),
        "abc" => Some(KeyOutput::Control(KeyboardControl::SwitchMode(
            KeyboardMode::TextLower,
        ))),
        "123" => Some(KeyOutput::Control(KeyboardControl::SwitchMode(
            KeyboardMode::Number,
        ))),
        " " => Some(KeyOutput::Char(' ')),
        s if s.chars().count() == 1 => Some(KeyOutput::Char(s.chars().next().unwrap())),
        _ => None,
    }
}

// ── Keyboard ──────────────────────────────────────────────────────────────────

/// Specialized button-grid keyboard with predefined mode maps (LPAR-13 §5.D).
///
/// Owns a [`ButtonMatrix`] internally.  Mode switching replaces the inner map.
/// Key output is delivered via the hook closure and/or the poll slot.
pub struct Keyboard {
    matrix: ButtonMatrix,
    mode: KeyboardMode,
    popovers: bool,
    /// Most-recently produced key output (poll slot).
    last_output: Option<KeyOutput>,
    /// Optional callback invoked on each key activation.
    hook: Option<Box<dyn FnMut(KeyOutput)>>,
}

impl Keyboard {
    /// Create a [`Keyboard`] occupying `bounds` in [`KeyboardMode::TextLower`].
    pub fn new(bounds: Rect) -> Self {
        let mut matrix = ButtonMatrix::new(bounds);
        matrix.set_map(MAP_LOWER);
        Self {
            matrix,
            mode: KeyboardMode::TextLower,
            popovers: false,
            last_output: None,
            hook: None,
        }
    }

    // ── Mode ──────────────────────────────────────────────────────────────

    /// Set the active key map mode and update the inner [`ButtonMatrix`] map.
    pub fn set_mode(&mut self, mode: KeyboardMode) {
        self.mode = mode;
        let map: &[&str] = match mode {
            KeyboardMode::TextLower => MAP_LOWER,
            KeyboardMode::TextUpper => MAP_UPPER,
            KeyboardMode::Number => MAP_NUMBER,
            KeyboardMode::Special => MAP_SPECIAL,
            // User-defined: no built-in map; leave current matrix unchanged.
            KeyboardMode::User1
            | KeyboardMode::User2
            | KeyboardMode::User3
            | KeyboardMode::User4 => return,
        };
        self.matrix.set_map(map);
    }

    /// Return the currently active mode.
    pub fn mode(&self) -> KeyboardMode {
        self.mode
    }

    // ── Popovers ──────────────────────────────────────────────────────────

    /// Store the popover preference (forwarded to the inner matrix; v1 drawing
    /// behavior may be a no-op pending LPAR-12 §13 deferred-Safe).
    pub fn set_popovers(&mut self, enable: bool) {
        self.popovers = enable;
    }

    /// Return whether popovers are requested.
    pub fn popovers(&self) -> bool {
        self.popovers
    }

    // ── Key output hook ───────────────────────────────────────────────────

    /// Install a callback invoked on every key activation.
    ///
    /// The hook is stored as `Box<dyn FnMut(KeyOutput)>` so it is independent
    /// of any specific text-field lifetime (LPAR-13 §5.D, `no_std + alloc`).
    pub fn set_key_output_hook<F>(&mut self, hook: F)
    where
        F: FnMut(KeyOutput) + 'static,
    {
        self.hook = Some(Box::new(hook));
    }

    /// Drain and return the most-recently produced [`KeyOutput`] since the last
    /// call.  Returns `None` if no key was pressed since the last poll.
    pub fn last_key_output(&mut self) -> Option<KeyOutput> {
        self.last_output.take()
    }

    // ── Key navigation helpers (LPAR-13 §5.J) ────────────────────────────

    /// Select the next navigable button (delegates to inner [`ButtonMatrix`]).
    ///
    /// Wire to `ObjectEvent::Key(Key::ArrowRight)` or `Key::Tab`.
    pub fn navigate_next(&mut self) {
        self.matrix.navigate_next();
    }

    /// Select the previous navigable button (delegates to inner [`ButtonMatrix`]).
    ///
    /// Wire to `ObjectEvent::Key(Key::ArrowLeft)` or `Key::BackTab`.
    pub fn navigate_prev(&mut self) {
        self.matrix.navigate_prev();
    }

    /// Activate the currently selected button (delegates to inner
    /// [`ButtonMatrix`]) and emit any resulting [`KeyOutput`].
    ///
    /// Wire to `ObjectEvent::Key(Key::Enter)`.
    pub fn activate_selected(&mut self) {
        let id = self.matrix.selected_button();
        if id == BUTTON_NONE {
            return;
        }
        // Snapshot label before mutable call
        let label_owned: String = self.matrix.button_text(id).unwrap_or_default().to_string();
        self.matrix.activate_selected();
        self.emit_for_label(&label_owned);
    }

    // ── Internal ──────────────────────────────────────────────────────────

    /// Emit a [`KeyOutput`] for the given label string, handling mode-switch
    /// side-effects inline.
    fn emit_for_label(&mut self, label: &str) {
        let output = match resolve_key(label) {
            Some(out) => out,
            None => return,
        };

        // Apply mode-switch side-effect before delivering to caller
        if let KeyOutput::Control(KeyboardControl::SwitchMode(new_mode)) = &output {
            let m = *new_mode;
            self.set_mode(m);
        }

        // Deliver via poll slot
        self.last_output = Some(output.clone());

        // Deliver via hook (takes ownership of output)
        if let Some(hook) = &mut self.hook {
            hook(output);
        }
    }

    /// Return a reference to the inner [`ButtonMatrix`] for style customization.
    pub fn matrix(&self) -> &ButtonMatrix {
        &self.matrix
    }

    /// Return a mutable reference to the inner [`ButtonMatrix`].
    pub fn matrix_mut(&mut self) -> &mut ButtonMatrix {
        &mut self.matrix
    }

    /// Return the currently selected button id in the inner matrix.
    pub fn selected_button(&self) -> ButtonId {
        self.matrix.selected_button()
    }
}

impl Widget for Keyboard {
    fn bounds(&self) -> Rect {
        self.matrix.bounds()
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.matrix.set_bounds(bounds);
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        self.matrix.draw(renderer);
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        let consumed = self.matrix.handle_event(event);
        if consumed && matches!(event, Event::PressRelease { .. }) {
            // The ButtonMatrix activated a button on PressRelease.
            // Emit the corresponding KeyOutput for the now-selected button.
            let activated_id = self.matrix.selected_button();
            if activated_id != BUTTON_NONE {
                let label_owned: String = self
                    .matrix
                    .button_text(activated_id)
                    .unwrap_or_default()
                    .to_string();
                self.emit_for_label(&label_owned);
            }
        }
        consumed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rlvgl_core::widget::{Color, Rect};

    fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    struct NullRenderer;
    impl rlvgl_core::renderer::Renderer for NullRenderer {
        fn fill_rect(&mut self, _: Rect, _: Color) {}
        fn draw_text(&mut self, _: (i32, i32), _: &str, _: Color) {}
    }

    #[test]
    fn new_starts_in_lower_mode() {
        let kb = Keyboard::new(rect(0, 0, 320, 160));
        assert_eq!(kb.mode(), KeyboardMode::TextLower);
    }

    #[test]
    fn set_mode_upper_changes_map() {
        let mut kb = Keyboard::new(rect(0, 0, 320, 160));
        kb.set_mode(KeyboardMode::TextUpper);
        assert_eq!(kb.mode(), KeyboardMode::TextUpper);
        // First button in upper map should be 'Q'
        assert_eq!(kb.matrix().button_text(ButtonId(0)), Some("Q"));
    }

    #[test]
    fn set_mode_number_changes_map() {
        let mut kb = Keyboard::new(rect(0, 0, 320, 160));
        kb.set_mode(KeyboardMode::Number);
        assert_eq!(kb.mode(), KeyboardMode::Number);
        assert_eq!(kb.matrix().button_text(ButtonId(0)), Some("1"));
    }

    #[test]
    fn set_mode_special_changes_map() {
        let mut kb = Keyboard::new(rect(0, 0, 320, 160));
        kb.set_mode(KeyboardMode::Special);
        assert_eq!(kb.mode(), KeyboardMode::Special);
    }

    #[test]
    fn activate_char_key_emits_char_output() {
        let mut kb = Keyboard::new(rect(0, 0, 320, 160));
        // Navigate to the 'q' key (button 0 in lower mode)
        kb.matrix_mut().set_selected_button(ButtonId(0));
        kb.activate_selected();
        assert_eq!(kb.last_key_output(), Some(KeyOutput::Char('q')));
        // Poll slot is drained
        assert_eq!(kb.last_key_output(), None);
    }

    #[test]
    fn activate_backspace_emits_backspace() {
        let mut kb = Keyboard::new(rect(0, 0, 320, 160));
        // Find backspace button id by scanning
        let count = kb.matrix().button_count();
        let mut bs_id = None;
        for i in 0..count {
            if kb.matrix().button_text(ButtonId(i as u16)) == Some("⌫") {
                bs_id = Some(ButtonId(i as u16));
                break;
            }
        }
        let bs_id = bs_id.expect("backspace button not found");
        kb.matrix_mut().set_selected_button(bs_id);
        kb.activate_selected();
        assert_eq!(kb.last_key_output(), Some(KeyOutput::Backspace));
    }

    #[test]
    fn activate_enter_emits_enter() {
        let mut kb = Keyboard::new(rect(0, 0, 320, 160));
        let count = kb.matrix().button_count();
        let mut enter_id = None;
        for i in 0..count {
            if kb.matrix().button_text(ButtonId(i as u16)) == Some("↵") {
                enter_id = Some(ButtonId(i as u16));
                break;
            }
        }
        let enter_id = enter_id.expect("enter button not found");
        kb.matrix_mut().set_selected_button(enter_id);
        kb.activate_selected();
        assert_eq!(kb.last_key_output(), Some(KeyOutput::Enter));
    }

    #[test]
    fn mode_switch_key_changes_mode() {
        let mut kb = Keyboard::new(rect(0, 0, 320, 160));
        // Find the ABC (switch to upper) button in lower mode
        let count = kb.matrix().button_count();
        let mut abc_id = None;
        for i in 0..count {
            if kb.matrix().button_text(ButtonId(i as u16)) == Some("ABC") {
                abc_id = Some(ButtonId(i as u16));
                break;
            }
        }
        let abc_id = abc_id.expect("ABC mode-switch button not found");
        kb.matrix_mut().set_selected_button(abc_id);
        kb.activate_selected();
        assert_eq!(kb.mode(), KeyboardMode::TextUpper);
    }

    #[test]
    fn hook_is_called_on_key_activation() {
        use alloc::rc::Rc;
        use core::cell::Cell;

        let called = Rc::new(Cell::new(false));
        let called2 = called.clone();

        let mut kb = Keyboard::new(rect(0, 0, 320, 160));
        kb.set_key_output_hook(move |_out| {
            called2.set(true);
        });
        kb.matrix_mut().set_selected_button(ButtonId(0));
        kb.activate_selected();
        assert!(called.get());
    }

    #[test]
    fn navigate_next_and_prev_work() {
        let mut kb = Keyboard::new(rect(0, 0, 320, 160));
        kb.navigate_next();
        assert_eq!(kb.selected_button(), ButtonId(0));
        kb.navigate_next();
        assert_eq!(kb.selected_button(), ButtonId(1));
        kb.navigate_prev();
        assert_eq!(kb.selected_button(), ButtonId(0));
    }

    #[test]
    fn set_bounds_propagates_to_matrix() {
        let mut kb = Keyboard::new(rect(0, 0, 320, 160));
        kb.set_bounds(rect(10, 20, 400, 200));
        assert_eq!(kb.bounds(), rect(10, 20, 400, 200));
    }

    #[test]
    fn draw_does_not_panic() {
        let kb = Keyboard::new(rect(0, 0, 320, 160));
        let mut r = NullRenderer;
        kb.draw(&mut r);
    }

    #[test]
    fn popovers_stored() {
        let mut kb = Keyboard::new(rect(0, 0, 320, 160));
        kb.set_popovers(true);
        assert!(kb.popovers());
        kb.set_popovers(false);
        assert!(!kb.popovers());
    }
}
