// SPDX-License-Identifier: MIT
//! Multi-line editable text area widget (LPAR-14 §5.C).
//!
//! [`Textarea`] is the `widgets`-layer v2 textarea.  It reuses
//! [`rlvgl_core::edit::EditCore`] for the mutation model and
//! [`rlvgl_core::font::wrap_greedy_ltr`] for multi-line wrapping —
//! it does **not** reimplement either.
//!
//! ## Key differences from `ui::input::Textarea` (WID-01)
//!
//! | Feature | `ui::input::Textarea` | `widgets::textarea::Textarea` |
//! |---|---|---|
//! | Multi-line wrap | `'\n'`-split only | LPAR-08 greedy wrap + hard newlines |
//! | Overflow scroll | no | `scroll_offset_y` |
//! | Placeholder | no | yes |
//! | Password mode | no | yes |
//! | One-line mode | no | yes |
//! | `Keyboard` binding | manual | `apply_key_output` |
//!
//! ## Usage
//!
//! ```ignore
//! let mut ta = Textarea::new(Rect { x: 10, y: 10, width: 200, height: 100 });
//! ta.set_placeholder("Type here…");
//! ta.set_active(true);
//! // Wire to a Keyboard:
//! keyboard.set_key_output_hook(move |ko| ta.apply_key_output(ko));
//! ```

use alloc::boxed::Box;
use alloc::string::String;
#[cfg(test)]
use rlvgl_core::bitmap_font::FONT_6X10;
use rlvgl_core::draw::draw_widget_bg;
use rlvgl_core::edit::{AcceptFn, CARET_WIDTH, ChangeCallback, EditCore};
use rlvgl_core::event::{Event, Key};
use rlvgl_core::font::{FontMetrics, WidgetFont, shape_text_ltr, wrap_greedy_ltr};
use rlvgl_core::renderer::{ClipRenderer, Renderer};
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

use crate::keyboard::KeyOutput;

/// Submit callback type for one-line mode.
type SubmitCallback = Box<dyn FnMut(&str)>;

/// Default masking glyph for password mode.
const PASSWORD_GLYPH: char = '*';

/// Multi-line editable text area widget (LPAR-14 §5.C).
///
/// Wraps [`EditCore`] (promoted to `rlvgl_core`) for the mutation model and
/// uses the LPAR-08 greedy-wrap algorithm for line layout.  Supports
/// placeholder text, password masking, optional single-line mode, and vertical
/// overflow scrolling.
///
/// # Key navigation
///
/// Wire a [`Keyboard`](crate::keyboard::Keyboard) hook to
/// [`apply_key_output`](Self::apply_key_output):
///
/// ```ignore
/// keyboard.set_key_output_hook(|ko| ta.apply_key_output(ko));
/// ```
///
/// For direct key injection (hardware keyboard / test harness) use
/// [`Widget::handle_event`] with [`Event::KeyDown`].
pub struct Textarea {
    /// Inner edit state (buffer, caret, mutation gates).
    core: EditCore,
    /// Outer bounds — also stored in `core.bounds` but kept here for the
    /// `Widget::bounds` fast path.
    bounds: Rect,
    /// Placeholder shown when the buffer is empty and the field is inactive.
    placeholder: Option<String>,
    /// When `true`, every character in the display is replaced by
    /// [`PASSWORD_GLYPH`] (buffer is unchanged).
    password_mode: bool,
    /// When `true`, `Enter` emits a submit signal rather than a newline, and
    /// wrapping is suppressed (display is a single scrollable line).
    one_line: bool,
    /// Vertical scroll offset in pixels (non-negative).
    scroll_offset_y: i32,
    /// Optional submit callback fired when `Enter` is pressed in one-line mode
    /// or via [`KeyOutput::Enter`] when `one_line` is active.
    on_submit: Option<SubmitCallback>,
    /// Widget style (background, border).
    pub style: Style,
    /// Text colour.
    pub text_color: Color,
    /// Font assignment for this widget (FONT-00 §5); resolves to `FONT_6X10`
    /// when unset.
    font: WidgetFont,
}

impl Textarea {
    /// Create a new `Textarea` occupying `bounds` with an empty buffer.
    pub fn new(bounds: Rect) -> Self {
        Self {
            core: EditCore::new("", bounds, true),
            bounds,
            placeholder: None,
            password_mode: false,
            one_line: false,
            scroll_offset_y: 0,
            on_submit: None,
            style: Style::default(),
            text_color: Color(0, 0, 0, 255),
            font: WidgetFont::new(),
        }
    }

    // ── Buffer ────────────────────────────────────────────────────────────────

    /// Replace the buffer with `text` programmatically.
    ///
    /// Bypasses the ASCII gate and `accept` predicate.  Clamps the caret to
    /// the new length.  Fires `on_change` if registered.
    pub fn set_text(&mut self, text: &str) {
        self.core.set_text(text);
        self.clamp_scroll();
    }

    /// Return a reference to the current buffer contents.
    pub fn text(&self) -> &str {
        &self.core.buffer
    }

    // ── Placeholder ───────────────────────────────────────────────────────────

    /// Set the placeholder string shown when the buffer is empty and the
    /// field is inactive.  Pass `""` to clear.
    pub fn set_placeholder(&mut self, text: &str) {
        self.placeholder = if text.is_empty() {
            None
        } else {
            Some(String::from(text))
        };
    }

    /// Return the current placeholder string, if any.
    pub fn placeholder(&self) -> Option<&str> {
        self.placeholder.as_deref()
    }

    // ── Password mode ─────────────────────────────────────────────────────────

    /// Enable or disable password masking.
    ///
    /// When enabled, every character in the rendered display is replaced by
    /// [`PASSWORD_GLYPH`] (`*`).  The buffer stores the real text.
    pub fn set_password_mode(&mut self, enable: bool) {
        self.password_mode = enable;
    }

    /// Return `true` if password masking is active.
    pub fn password_mode(&self) -> bool {
        self.password_mode
    }

    // ── One-line mode ─────────────────────────────────────────────────────────

    /// Enable or disable single-line mode.
    ///
    /// In one-line mode:
    /// * `'\n'` is never inserted; Enter fires `on_submit` instead.
    /// * Wrapping is suppressed; the content scrolls horizontally.
    pub fn set_one_line(&mut self, enable: bool) {
        self.one_line = enable;
        // Sync the core's multi_line flag: multi_line == !one_line.
        self.core.multi_line = !enable;
    }

    /// Return `true` if single-line mode is active.
    pub fn one_line(&self) -> bool {
        self.one_line
    }

    // ── Focus / activation ────────────────────────────────────────────────────

    /// Return `true` when this field is consuming keyboard events.
    pub fn is_active(&self) -> bool {
        self.core.active
    }

    /// Toggle key consumption.  Applications keep at most one field active;
    /// no focus framework is imposed (WID-00 §7.1).
    pub fn set_active(&mut self, active: bool) {
        self.core.active = active;
    }

    // ── Caret ─────────────────────────────────────────────────────────────────

    /// Return the current caret position (char index, `0..=char_count`).
    pub fn caret(&self) -> usize {
        self.core.caret
    }

    // ── Style ─────────────────────────────────────────────────────────────────

    /// Immutable access to the widget style.
    pub fn widget_style(&self) -> &Style {
        &self.style
    }

    /// Mutable access to the widget style.
    pub fn widget_style_mut(&mut self) -> &mut Style {
        &mut self.style
    }

    /// Assign the font used to render this widget (FONT-00 §5); resolves to
    /// `FONT_6X10` when unset.
    pub fn set_font(&mut self, font: &'static dyn FontMetrics) {
        self.font.set(font);
    }

    // ── Mutation gates ────────────────────────────────────────────────────────

    /// Register a change callback invoked after every committed edit and on
    /// programmatic [`set_text`](Self::set_text) calls.
    pub fn on_change<F: FnMut(&str) + 'static>(mut self, handler: F) -> Self {
        self.core.on_change = Some(Box::new(handler) as ChangeCallback);
        self
    }

    /// Register a submit callback fired when Enter is pressed in one-line
    /// mode.
    pub fn on_submit<F: FnMut(&str) + 'static>(mut self, handler: F) -> Self {
        self.on_submit = Some(Box::new(handler));
        self
    }

    /// Install an accepted-charset predicate (evaluated after the ASCII gate).
    pub fn with_accept<F: Fn(char) -> bool + 'static>(mut self, accept: F) -> Self {
        self.core.accept = Some(Box::new(accept) as AcceptFn);
        self
    }

    /// Cap the buffer at `max` characters.
    pub fn with_max_len(mut self, max: usize) -> Self {
        self.core.max_len = Some(max);
        self
    }

    // ── Keyboard binding (§5.I) ───────────────────────────────────────────────

    /// Apply a [`KeyOutput`] value produced by a [`Keyboard`] widget.
    ///
    /// Mapping (LPAR-14 §5.I):
    ///
    /// | `KeyOutput` | one-line=false | one-line=true |
    /// |---|---|---|
    /// | `Char(c)` | `try_insert(c)` | `try_insert(c)` |
    /// | `Backspace` | `try_backspace()` | `try_backspace()` |
    /// | `Enter` | `try_insert('\n')` | fire `on_submit`, no mutation |
    /// | `Tab` | ignored | ignored |
    /// | `Escape` | deactivate | deactivate |
    /// | `Control(_)` | ignored (mode handled by Keyboard) | same |
    pub fn apply_key_output(&mut self, ko: KeyOutput) {
        match ko {
            KeyOutput::Char(c) => {
                self.core.try_insert(c);
                self.clamp_scroll();
            }
            KeyOutput::Backspace => {
                self.core.try_backspace();
                self.clamp_scroll();
            }
            KeyOutput::Enter => {
                if self.one_line {
                    if let Some(cb) = self.on_submit.as_mut() {
                        cb(&self.core.buffer);
                    }
                } else {
                    self.core.try_insert('\n');
                    self.clamp_scroll();
                }
            }
            KeyOutput::Escape => {
                self.core.active = false;
            }
            // Tab and Control are silently ignored at v1.
            KeyOutput::Tab | KeyOutput::Control(_) => {}
        }
    }

    // ── Scroll ────────────────────────────────────────────────────────────────

    /// Return the current vertical scroll offset in pixels.
    pub fn scroll_offset_y(&self) -> i32 {
        self.scroll_offset_y
    }

    /// Clamp `scroll_offset_y` so the caret remains visible inside `bounds`.
    fn clamp_scroll(&mut self) {
        let line_h = self.core.line_height;
        let visible_h = self.bounds.height;
        let (row, _) = self.core.caret_row_col();
        let caret_top = row * line_h;
        let caret_bottom = caret_top + line_h;

        // Scroll down if caret is below the visible area.
        if caret_bottom > self.scroll_offset_y + visible_h {
            self.scroll_offset_y = caret_bottom - visible_h;
        }
        // Scroll up if caret is above the visible area.
        if caret_top < self.scroll_offset_y {
            self.scroll_offset_y = caret_top;
        }
        // Never scroll past the top.
        if self.scroll_offset_y < 0 {
            self.scroll_offset_y = 0;
        }
    }

    // ── Rendering helpers ─────────────────────────────────────────────────────

    /// Return the display string: either the real buffer or a password-masked
    /// version of the same length.
    fn display_text(&self) -> String {
        if self.password_mode {
            self.core.buffer.chars().map(|_| PASSWORD_GLYPH).collect()
        } else {
            self.core.buffer.clone()
        }
    }

    /// Draw shaped text for a single line into `renderer`, clipped to `clip`.
    fn draw_line(&self, renderer: &mut dyn Renderer, line: &str, baseline: i32, clip: Rect) {
        if line.is_empty() {
            return;
        }
        let font = self.font.resolve();
        let shaped = shape_text_ltr(font, line, (self.bounds.x, baseline), 0);
        let mut clip_r = ClipRenderer::new(renderer, clip);
        clip_r.draw_text_shaped(&shaped, (0, 0), self.text_color);
    }

    /// Draw the caret line if the field is active.
    fn draw_caret(&self, renderer: &mut dyn Renderer) {
        if !self.core.active {
            return;
        }
        let (row, col) = self.core.caret_row_col();
        let x = self.bounds.x + col * self.core.char_width;
        let y = self.bounds.y + row * self.core.line_height - self.scroll_offset_y;
        // Only draw caret when it is inside the visible clip.
        if y + self.core.line_height <= self.bounds.y || y >= self.bounds.y + self.bounds.height {
            return;
        }
        renderer.fill_rect(
            Rect {
                x,
                y,
                width: CARET_WIDTH,
                height: self.core.line_height,
            },
            self.style.border_color,
        );
    }
}

impl Widget for Textarea {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_bounds(&mut self, new_bounds: Rect) {
        self.bounds = new_bounds;
        self.core.bounds = new_bounds;
        // Reflow: clamp scroll after bounds change.
        self.clamp_scroll();
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        draw_widget_bg(renderer, self.bounds, &self.style);

        let clip = self.bounds;
        let buf = self.display_text();
        let font = self.font.resolve();

        // Placeholder path: show placeholder when buffer is empty and inactive.
        if buf.is_empty() && !self.core.active {
            if let Some(ph) = self.placeholder.as_deref() {
                let ph_color = Color(
                    self.text_color.0,
                    self.text_color.1,
                    self.text_color.2,
                    self.text_color.3 / 2,
                );
                let lm = font.line_metrics();
                let baseline = self.bounds.y + lm.ascent as i32 - self.scroll_offset_y;
                let shaped = shape_text_ltr(font, ph, (self.bounds.x, baseline), 0);
                let mut clip_r = ClipRenderer::new(renderer, clip);
                clip_r.draw_text_shaped(&shaped, (0, 0), ph_color);
            }
            return;
        }

        if self.one_line {
            // Single-line: draw the buffer as one unwrapped line.
            let lm = font.line_metrics();
            let baseline = self.bounds.y + lm.ascent as i32 - self.scroll_offset_y;
            self.draw_line(renderer, &buf, baseline, clip);
        } else {
            // Multi-line: use the LPAR-08 greedy wrap.
            let lm = font.line_metrics();
            let line_h = lm.line_height as i32;
            let wrapped = wrap_greedy_ltr(font, &buf, self.bounds.width, 0, 0);
            for (i, wl) in wrapped.lines.iter().enumerate() {
                let top = i as i32 * line_h - self.scroll_offset_y;
                // Skip lines that are entirely outside the visible area.
                if top + line_h <= 0 || top >= self.bounds.height {
                    continue;
                }
                let baseline = self.bounds.y + top + lm.ascent as i32;
                let segment = &buf[wl.start..wl.end];
                self.draw_line(renderer, segment, baseline, clip);
            }
        }

        self.draw_caret(renderer);
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if !self.core.active {
            return false;
        }
        if let Event::KeyDown { key } = event {
            match key {
                Key::Enter => {
                    if self.one_line {
                        if let Some(cb) = self.on_submit.as_mut() {
                            cb(&self.core.buffer);
                        }
                    } else {
                        self.core.try_insert('\n');
                        self.clamp_scroll();
                    }
                    return true;
                }
                other => {
                    let consumed = self.core.handle_key(other);
                    if consumed {
                        self.clamp_scroll();
                    }
                    return consumed;
                }
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rlvgl_core::widget::Rect;

    const BOUNDS: Rect = Rect {
        x: 0,
        y: 0,
        width: 200,
        height: 100,
    };

    fn narrow() -> Rect {
        // Width just fits ~3 chars of FONT_6X10 (advance ≈ 6 px each).
        Rect {
            x: 0,
            y: 0,
            width: 24,
            height: 200,
        }
    }

    // ── apply_key_output ─────────────────────────────────────────────────────

    #[test]
    fn apply_char_inserts_into_buffer() {
        let mut ta = Textarea::new(BOUNDS);
        ta.set_active(true);
        ta.apply_key_output(KeyOutput::Char('H'));
        ta.apply_key_output(KeyOutput::Char('i'));
        assert_eq!(ta.text(), "Hi");
        assert_eq!(ta.caret(), 2);
    }

    #[test]
    fn apply_backspace_removes_last_char() {
        let mut ta = Textarea::new(BOUNDS);
        ta.set_active(true);
        ta.apply_key_output(KeyOutput::Char('X'));
        ta.apply_key_output(KeyOutput::Backspace);
        assert_eq!(ta.text(), "");
        assert_eq!(ta.caret(), 0);
    }

    #[test]
    fn apply_enter_inserts_newline_in_multiline_mode() {
        let mut ta = Textarea::new(BOUNDS);
        ta.set_active(true);
        ta.apply_key_output(KeyOutput::Char('A'));
        ta.apply_key_output(KeyOutput::Enter);
        ta.apply_key_output(KeyOutput::Char('B'));
        assert_eq!(ta.text(), "A\nB");
    }

    #[test]
    fn apply_enter_fires_submit_in_one_line_mode() {
        use alloc::rc::Rc;
        use alloc::string::String;
        use alloc::vec::Vec;
        use core::cell::RefCell;

        let log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let log2 = log.clone();
        let mut ta =
            Textarea::new(BOUNDS).on_submit(move |s| log2.borrow_mut().push(String::from(s)));
        ta.set_one_line(true);
        ta.set_active(true);
        ta.apply_key_output(KeyOutput::Char('O'));
        ta.apply_key_output(KeyOutput::Char('K'));
        ta.apply_key_output(KeyOutput::Enter);
        assert_eq!(*log.borrow(), alloc::vec!["OK"]);
        // Buffer is not mutated by Enter in one-line mode.
        assert_eq!(ta.text(), "OK");
    }

    #[test]
    fn apply_escape_deactivates() {
        let mut ta = Textarea::new(BOUNDS);
        ta.set_active(true);
        ta.apply_key_output(KeyOutput::Escape);
        assert!(!ta.is_active());
    }

    #[test]
    fn apply_tab_and_control_are_ignored() {
        let mut ta = Textarea::new(BOUNDS);
        ta.set_active(true);
        ta.apply_key_output(KeyOutput::Tab);
        assert_eq!(ta.text(), "");
        ta.apply_key_output(KeyOutput::Control(
            crate::keyboard::KeyboardControl::SwitchMode(crate::keyboard::KeyboardMode::Number),
        ));
        assert_eq!(ta.text(), "");
    }

    // ── Placeholder ───────────────────────────────────────────────────────────

    #[test]
    fn placeholder_set_and_clear() {
        let mut ta = Textarea::new(BOUNDS);
        ta.set_placeholder("hint");
        assert_eq!(ta.placeholder(), Some("hint"));
        ta.set_placeholder("");
        assert_eq!(ta.placeholder(), None);
    }

    // ── Password mode ─────────────────────────────────────────────────────────

    #[test]
    fn password_mode_masks_display_text() {
        let mut ta = Textarea::new(BOUNDS);
        ta.set_password_mode(true);
        ta.set_text("secret");
        assert_eq!(ta.text(), "secret");
        assert_eq!(ta.display_text(), "******");
    }

    // ── One-line mode ─────────────────────────────────────────────────────────

    #[test]
    fn one_line_prevents_newline_insertion() {
        let mut ta = Textarea::new(BOUNDS);
        ta.set_one_line(true);
        ta.set_active(true);
        ta.handle_event(&Event::KeyDown { key: Key::Enter });
        // Enter should NOT insert '\n' in one-line mode.
        assert_eq!(ta.text(), "");
    }

    // ── Wrapping / scroll ─────────────────────────────────────────────────────

    #[test]
    fn wrap_reflow_on_narrow_bounds() {
        let mut ta = Textarea::new(narrow());
        ta.set_active(true);
        // Fill enough text to force wrapping.
        ta.set_text("abcdefghij");
        // The wrap engine must produce multiple lines for a 24 px wide box
        // with FONT_6X10 (≈6 px/char → 4 chars/line max).
        let wrapped = wrap_greedy_ltr(&FONT_6X10, ta.text(), 24, 0, 0);
        assert!(
            wrapped.lines.len() > 1,
            "expected wrapping for narrow bounds, got {} lines",
            wrapped.lines.len()
        );
    }

    #[test]
    fn scroll_advances_on_overflow() {
        let bounds = Rect {
            x: 0,
            y: 0,
            width: 200,
            height: 32, // only fits 2 lines of FONT_6X10 (16 px each)
        };
        let mut ta = Textarea::new(bounds);
        ta.set_active(true);
        // Force 3+ lines worth of text via hard newlines.
        for _ in 0..4 {
            ta.apply_key_output(KeyOutput::Char('A'));
            ta.apply_key_output(KeyOutput::Enter);
        }
        assert!(
            ta.scroll_offset_y() > 0,
            "expected positive scroll, got {}",
            ta.scroll_offset_y()
        );
    }
}
