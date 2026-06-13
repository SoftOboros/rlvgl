// SPDX-License-Identifier: MIT
//! Shared edit-state machine promoted from `rlvgl-ui` (LPAR-14 §5.C).
//!
//! [`EditCore`] holds the edit buffer, caret, and mutation gates that are
//! shared between single-line [`Input`](rlvgl_ui::input::Input), the
//! `ui`-layer [`Textarea`](rlvgl_ui::input::Textarea), and the new
//! `widgets`-layer [`Textarea`](crate::edit) (LPAR-14).
//!
//! The promotion from `pub(crate)` in `rlvgl-ui` to `pub` in `rlvgl-core`
//! breaks the crate-cycle that would arise if `rlvgl-widgets::textarea`
//! depended on `rlvgl-ui` (which itself depends on `rlvgl-widgets`).
//!
//! # WID-00 compatibility guarantee
//!
//! Every method on [`EditCore`] preserves its WID-00 / WID-01 contract.  No
//! public behaviour changes are permitted without a ratified LPAR chapter.

use alloc::boxed::Box;
use alloc::string::String;

use crate::widget::Rect;

// Re-exported so that crates that previously used constants from `rlvgl_ui::input`
// can import them from either location.

/// Default nominal character advance for caret geometry (WID-00 §6.3).
pub const DEFAULT_CHAR_WIDTH: i32 = 8;
/// Default nominal line height for caret geometry (WID-00 §6.3).
pub const DEFAULT_LINE_HEIGHT: i32 = 16;
/// Caret thickness in pixels.
pub const CARET_WIDTH: i32 = 2;

/// Callback type invoked when the edit buffer changes.
pub type ChangeCallback = Box<dyn FnMut(&str)>;
/// Accepted-charset predicate (evaluated after the printable-ASCII gate).
pub type AcceptFn = Box<dyn Fn(char) -> bool>;

/// Shared edit-state machine for single-line and multi-line text fields
/// (WID-00 §5; LPAR-14 §5.C).
///
/// `EditCore` owns the edit buffer, caret position, and the optional
/// mutation gates (`max_len`, `accept`, `on_change`).  Rendering helpers
/// that need glyph geometry or a [`Label`] remain in the widget wrappers
/// that embed this struct.
///
/// # Buffer and caret invariants
///
/// * The buffer may contain arbitrary Unicode inserted via [`set_text`].
/// * Interactive insertion via [`try_insert`] is limited to printable ASCII
///   (`0x20..=0x7E`) plus `'\n'` on multi-line fields (WID-00 §5.2).
/// * `caret` is a **char** index (`0..=char_count()`).  [`byte_index`]
///   converts it to a byte offset for safe `String` mutation.
///
/// [`set_text`]: EditCore::set_text
/// [`try_insert`]: EditCore::try_insert
/// [`byte_index`]: EditCore::byte_index
pub struct EditCore {
    /// Text bounds used for caret geometry and derived rendering.
    ///
    /// Widgets that embed `EditCore` expose `set_bounds` on themselves and
    /// forward the update here.
    pub bounds: Rect,
    /// Edit buffer.
    ///
    /// ASCII-bounded for keyboard input so byte index == char index in
    /// practice; stored as `String` so a programmatic `set_text` can hold
    /// arbitrary Unicode without corrupting subsequent edits.
    pub buffer: String,
    /// Caret position as a char index in `0..=char_count()`.
    pub caret: usize,
    /// Whether this field is currently consuming keyboard events.
    pub active: bool,
    /// Whether Enter inserts a newline (`true`) or signals submit (`false`).
    pub multi_line: bool,
    /// Optional cap on the number of characters in the buffer.
    pub max_len: Option<usize>,
    /// Optional accepted-charset predicate (applied after the ASCII gate).
    pub accept: Option<AcceptFn>,
    /// Optional change callback invoked after every committed edit.
    pub on_change: Option<ChangeCallback>,
    /// Nominal character advance used for caret geometry (pixels).
    pub char_width: i32,
    /// Nominal line height used for caret geometry and line pitch (pixels).
    pub line_height: i32,
}

impl EditCore {
    /// Create an `EditCore` with `text` pre-loaded, caret at the end.
    ///
    /// `multi_line` controls whether Enter inserts a newline.
    pub fn new(text: &str, bounds: Rect, multi_line: bool) -> Self {
        let buffer = String::from(text);
        let caret = buffer.chars().count();
        Self {
            bounds,
            buffer,
            caret,
            active: false,
            multi_line,
            max_len: None,
            accept: None,
            on_change: None,
            char_width: DEFAULT_CHAR_WIDTH,
            line_height: DEFAULT_LINE_HEIGHT,
        }
    }

    /// Replace the buffer with `text`, clamp the caret, fire `on_change`.
    ///
    /// This is the programmatic path; it bypasses the ASCII gate and
    /// `accept` predicate.
    pub fn set_text(&mut self, text: &str) {
        self.buffer = String::from(text);
        self.caret = self.caret.min(self.buffer.chars().count());
        if let Some(cb) = self.on_change.as_mut() {
            cb(&self.buffer);
        }
    }

    /// Sync the label/display state and fire `on_change` after a committed
    /// edit (WID-00 §5.1).
    ///
    /// Called internally by [`try_insert`] and [`try_backspace`]; also
    /// available for widget wrappers that perform their own buffer mutations.
    ///
    /// [`try_insert`]: Self::try_insert
    /// [`try_backspace`]: Self::try_backspace
    pub fn committed(&mut self) {
        if let Some(cb) = self.on_change.as_mut() {
            cb(&self.buffer);
        }
    }

    /// Convert a char index to a byte offset in [`Self::buffer`].
    ///
    /// Safe for any char index in `0..=char_count()`; returns
    /// `buffer.len()` for out-of-range values.
    pub fn byte_index(&self, char_index: usize) -> usize {
        self.buffer
            .char_indices()
            .nth(char_index)
            .map(|(i, _)| i)
            .unwrap_or(self.buffer.len())
    }

    /// Return the number of chars currently in the buffer.
    pub fn char_count(&self) -> usize {
        self.buffer.chars().count()
    }

    /// Attempt to insert `c` at the caret.
    ///
    /// Returns `true` when the edit was applied.  Failed insertions leave
    /// the buffer and caret untouched and do **not** fire `on_change`
    /// (WID-00 §5.1).
    ///
    /// Gate order:
    /// 1. ASCII printable bound (`0x20..=0x7E`) plus `'\n'` on multi-line.
    /// 2. `accept` predicate (skipped for `'\n'`).
    /// 3. `max_len` cap.
    pub fn try_insert(&mut self, c: char) -> bool {
        let printable = ('\u{20}'..='\u{7e}').contains(&c);
        if !(printable || (c == '\n' && self.multi_line)) {
            return false;
        }
        if c != '\n'
            && let Some(accept) = self.accept.as_ref()
            && !accept(c)
        {
            return false;
        }
        if let Some(max) = self.max_len
            && self.char_count() >= max
        {
            return false;
        }
        let at = self.byte_index(self.caret);
        self.buffer.insert(at, c);
        self.caret += 1;
        self.committed();
        true
    }

    /// Delete the character before the caret.
    ///
    /// Returns `true` if a character was removed.  No-ops silently at
    /// position 0 (WID-00 §5.1).
    pub fn try_backspace(&mut self) -> bool {
        if self.caret == 0 {
            return false;
        }
        let at = self.byte_index(self.caret - 1);
        self.buffer.remove(at);
        self.caret -= 1;
        self.committed();
        true
    }

    /// Handle a raw key event while the field is active.
    ///
    /// Returns `true` when the key was consumed.  `Enter` is **not**
    /// handled here — its semantics differ between single-line (`Input`
    /// fires submit) and multi-line (`Textarea` inserts newline), so the
    /// wrapper is responsible (WID-00 §5.3).
    pub fn handle_key(&mut self, key: &crate::event::Key) -> bool {
        use crate::event::Key;
        match key {
            Key::Character(c) => {
                self.try_insert(*c);
                true // consumed even when rejected: the key targeted us
            }
            Key::Space => {
                self.try_insert(' ');
                true
            }
            Key::Backspace => {
                self.try_backspace();
                true
            }
            Key::ArrowLeft => {
                self.caret = self.caret.saturating_sub(1);
                true
            }
            Key::ArrowRight => {
                self.caret = (self.caret + 1).min(self.char_count());
                true
            }
            _ => false,
        }
    }

    /// Return the `(row, col)` of the caret in `'\n'`-split line space.
    ///
    /// Row 0 is the first line; col 0 is the start of a line.
    pub fn caret_row_col(&self) -> (i32, i32) {
        let mut row = 0i32;
        let mut col = 0i32;
        for c in self.buffer.chars().take(self.caret) {
            if c == '\n' {
                row += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        (row, col)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Key;
    use crate::widget::Rect;

    const BOUNDS: Rect = Rect {
        x: 0,
        y: 0,
        width: 200,
        height: 100,
    };

    fn core(multi: bool) -> EditCore {
        EditCore::new("", BOUNDS, multi)
    }

    #[test]
    fn insert_printable_ascii() {
        let mut ec = core(false);
        ec.active = true;
        assert!(ec.try_insert('A'));
        assert_eq!(ec.buffer, "A");
        assert_eq!(ec.caret, 1);
    }

    #[test]
    fn insert_newline_only_on_multiline() {
        let mut single = core(false);
        let mut multi = core(true);
        assert!(!single.try_insert('\n'));
        assert!(multi.try_insert('\n'));
    }

    #[test]
    fn backspace_removes_char_and_decrements_caret() {
        let mut ec = core(false);
        ec.try_insert('X');
        assert!(ec.try_backspace());
        assert_eq!(ec.buffer, "");
        assert_eq!(ec.caret, 0);
        // no-op at position 0
        assert!(!ec.try_backspace());
    }

    #[test]
    fn handle_key_arrow_navigation() {
        let mut ec = core(false);
        ec.try_insert('A');
        ec.try_insert('B');
        ec.handle_key(&Key::ArrowLeft);
        assert_eq!(ec.caret, 1);
        ec.handle_key(&Key::ArrowLeft);
        assert_eq!(ec.caret, 0);
        // clamp at 0
        ec.handle_key(&Key::ArrowLeft);
        assert_eq!(ec.caret, 0);
        ec.handle_key(&Key::ArrowRight);
        assert_eq!(ec.caret, 1);
        ec.handle_key(&Key::ArrowRight);
        ec.handle_key(&Key::ArrowRight);
        assert_eq!(ec.caret, 2, "clamped at end");
    }

    #[test]
    fn max_len_gate() {
        let mut ec = core(false);
        ec.max_len = Some(2);
        ec.try_insert('A');
        ec.try_insert('B');
        assert!(!ec.try_insert('C'));
        assert_eq!(ec.buffer, "AB");
    }

    #[test]
    fn accept_gate() {
        let mut ec = core(false);
        ec.accept = Some(Box::new(|c: char| c.is_ascii_digit()));
        assert!(ec.try_insert('5'));
        assert!(!ec.try_insert('x'));
        assert_eq!(ec.buffer, "5");
    }

    #[test]
    fn set_text_bypasses_ascii_gate_and_fires_callback() {
        use alloc::rc::Rc;
        use alloc::string::String;
        use alloc::vec::Vec;
        use core::cell::RefCell;
        let log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let log2 = log.clone();
        let mut ec = core(false);
        ec.on_change = Some(Box::new(move |s| log2.borrow_mut().push(String::from(s))));
        ec.set_text("héllo");
        assert_eq!(ec.buffer, "héllo");
        assert_eq!(log.borrow()[0], "héllo");
    }

    #[test]
    fn caret_row_col_multiline() {
        let mut ec = core(true);
        ec.try_insert('A');
        ec.try_insert('B');
        ec.try_insert('\n');
        ec.try_insert('C');
        assert_eq!(ec.caret_row_col(), (1, 1));
    }

    #[test]
    fn byte_index_is_char_aware() {
        let mut ec = core(false);
        // Insert via set_text to bypass ASCII gate
        ec.set_text("abc");
        assert_eq!(ec.byte_index(0), 0);
        assert_eq!(ec.byte_index(1), 1);
        assert_eq!(ec.byte_index(3), 3);
    }
}
