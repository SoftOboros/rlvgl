// SPDX-License-Identifier: MIT
//! Editable input and textarea components for rlvgl-ui (WID initiative).
//!
//! [`Input`] (single-line) and [`Textarea`] (multi-line) own an edit
//! buffer and, while **active** ([`Input::set_active`]), consume
//! [`Event::KeyDown`]: printable-ASCII insertion, [`Key::Backspace`]
//! deletion, Enter (submit / newline), and Left/Right caret movement —
//! with a visible line caret, change/submit callbacks, and max-length +
//! accepted-charset hooks (e.g. a digits-plus-`*`/`#` dialpad field).
//!
//! Key routing is deliberately minimal (WID-00 §7): applications toggle
//! `set_active` from their own focus bookkeeping; no focus framework is
//! imposed. Inactive fields consume nothing and never mutate.
//!
//! Rendering still delegates text to [`rlvgl_widgets::label::Label`].
//! Caret geometry uses nominal char metrics (WID-00 §6.3) because the
//! `Renderer` trait exposes no glyph extents — exact for monospace /
//! bitmap fonts, approximate for proportional backend fonts. The caret
//! does not blink (WID-00 §6.2): rendering stays a pure function of the
//! edit state, which headless D-dump tests rely on.
//!
//! # LPAR-14 §5.C — EditCore promotion
//!
//! [`EditCore`] has been moved to [`rlvgl_core::edit::EditCore`] to break the
//! crate cycle that would arise if `rlvgl-widgets::textarea` depended on
//! `rlvgl-ui`.  This module re-exports it as `pub use` so that all existing
//! `ui::input::EditCore` paths continue to resolve unchanged (WID-01 path
//! preserved).

use alloc::boxed::Box;
use rlvgl_core::{
    edit::EditCore as CoreEditCore,
    event::{Event, Key},
    font::{FontMetrics, WidgetFont, shape_text_ltr},
    renderer::Renderer,
    widget::{Rect, Widget},
};
use rlvgl_widgets::label::Label;

// ── Re-export so existing `ui::input::EditCore` paths keep working ────────────

/// Re-export of [`rlvgl_core::edit::EditCore`] for backward compatibility.
///
/// Widgets and test code that previously used `rlvgl_ui::input::EditCore`
/// continue to resolve here.  New code should import from
/// [`rlvgl_core::edit`] directly.
pub use rlvgl_core::edit::EditCore;

/// Re-exported default char width constant (WID-00 §6.3).
pub use rlvgl_core::edit::DEFAULT_CHAR_WIDTH;
/// Re-exported default line height constant (WID-00 §6.3).
pub use rlvgl_core::edit::DEFAULT_LINE_HEIGHT;

/// Callback type invoked when a single-line input submits (Enter).
type SubmitCallback = Box<dyn FnMut(&str)>;

/// Caret thickness in pixels.
const CARET_WIDTH: i32 = rlvgl_core::edit::CARET_WIDTH;

// ── UiEditCore: ui-layer wrapper that also owns a Label ───────────────────────
//
// The promoted `CoreEditCore` (from `rlvgl_core::edit`) does not own a
// `Label` — it holds only the buffer, caret, and mutation state.  The
// ui-layer widgets still need a `Label` for rendering and style access, so
// we keep a thin local wrapper struct `UiEditCore` that forwards all mutation
// calls to the inner `CoreEditCore` while keeping the `Label` in sync.

struct UiEditCore {
    core: CoreEditCore,
    label: Label,
}

impl UiEditCore {
    fn new(text: &str, bounds: Rect, multi_line: bool) -> Self {
        Self {
            core: CoreEditCore::new(text, bounds, multi_line),
            label: Label::new(text, bounds),
        }
    }

    fn set_font(&mut self, font: &'static dyn FontMetrics) {
        self.label.set_font(font);
    }

    fn widget_font_mut(&mut self) -> Option<&mut WidgetFont> {
        self.label.widget_font_mut()
    }

    fn set_text(&mut self, text: &str) {
        // CoreEditCore::set_text fires on_change; we also sync the label.
        self.core.buffer = alloc::string::String::from(text);
        self.core.caret = self.core.caret.min(self.core.buffer.chars().count());
        self.label.set_text(text);
        if let Some(cb) = self.core.on_change.as_mut() {
            cb(&self.core.buffer);
        }
    }

    fn committed(&mut self) {
        // Sync the label then fire on_change (mirrors original EditCore::committed).
        self.label.set_text(self.core.buffer.clone());
        if let Some(cb) = self.core.on_change.as_mut() {
            cb(&self.core.buffer);
        }
    }

    fn try_insert(&mut self, c: char) -> bool {
        // Delegate the gate logic to CoreEditCore but intercept the callback
        // so we can also sync the label.
        let printable = ('\u{20}'..='\u{7e}').contains(&c);
        if !(printable || (c == '\n' && self.core.multi_line)) {
            return false;
        }
        if c != '\n'
            && let Some(accept) = self.core.accept.as_ref()
            && !accept(c)
        {
            return false;
        }
        if let Some(max) = self.core.max_len
            && self.core.char_count() >= max
        {
            return false;
        }
        let at = self.core.byte_index(self.core.caret);
        self.core.buffer.insert(at, c);
        self.core.caret += 1;
        self.committed();
        true
    }

    fn try_backspace(&mut self) -> bool {
        if self.core.caret == 0 {
            return false;
        }
        let at = self.core.byte_index(self.core.caret - 1);
        self.core.buffer.remove(at);
        self.core.caret -= 1;
        self.committed();
        true
    }

    fn handle_key(&mut self, key: &Key) -> bool {
        match key {
            Key::Character(c) => {
                self.try_insert(*c);
                true
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
                self.core.caret = self.core.caret.saturating_sub(1);
                true
            }
            Key::ArrowRight => {
                self.core.caret = (self.core.caret + 1).min(self.core.char_count());
                true
            }
            _ => false,
        }
    }

    fn caret_row_col(&self) -> (i32, i32) {
        self.core.caret_row_col()
    }

    fn draw_caret(&self, renderer: &mut dyn Renderer) {
        if !self.core.active {
            return;
        }
        let bounds = self.label.bounds();
        let (row, col) = self.caret_row_col();
        renderer.fill_rect(
            Rect {
                x: bounds.x + col * self.core.char_width,
                y: bounds.y + row * self.core.line_height,
                width: CARET_WIDTH,
                height: self.core.line_height,
            },
            self.label.style.border_color,
        );
    }

    fn draw_multi_line(&self, renderer: &mut dyn Renderer) {
        rlvgl_core::draw::draw_widget_bg(renderer, self.label.bounds(), &self.label.style);
        let bounds = self.label.bounds();
        for (row, line) in self.core.buffer.split('\n').enumerate() {
            if line.is_empty() {
                continue;
            }
            let baseline = bounds.y + (row as i32 + 1) * self.core.line_height;
            let shaped = shape_text_ltr(self.label.resolved_font(), line, (bounds.x, baseline), 0);
            renderer.draw_text_shaped(
                &shaped,
                (0, 0),
                self.label.text_color_with_alpha(self.label.style.alpha),
            );
        }
    }
}

/// Single-line editable text input (WID-00 §5–§8).
///
/// While active (see [`set_active`](Self::set_active)), consumes
/// `KeyDown` for character insertion, backspace, caret movement, and
/// Enter (which fires [`on_submit`](Self::on_submit) without mutating
/// the buffer).
#[allow(clippy::type_complexity)]
pub struct Input {
    core: UiEditCore,
    on_submit: Option<SubmitCallback>,
}

impl Input {
    /// Create a new input with the provided initial value and bounds.
    /// The caret starts at the end of the initial text.
    pub fn new(text: &str, bounds: Rect) -> Self {
        Self {
            core: UiEditCore::new(text, bounds, false),
            on_submit: None,
        }
    }

    /// Assign the font used to render this input (FONT-00 §5); resolves to
    /// `FONT_6X10` when unset.
    pub fn set_font(&mut self, font: &'static dyn FontMetrics) {
        self.core.set_font(font);
    }

    /// Register a change handler invoked after every successful edit
    /// and on [`Self::set_text`].
    pub fn on_change<F: FnMut(&str) + 'static>(mut self, handler: F) -> Self {
        self.core.core.on_change = Some(Box::new(handler));
        self
    }

    /// Register a submit handler fired when Enter is pressed while
    /// active. The buffer is not mutated by Enter.
    pub fn on_submit<F: FnMut(&str) + 'static>(mut self, handler: F) -> Self {
        self.on_submit = Some(Box::new(handler));
        self
    }

    /// Set whether this input consumes key events and return the widget.
    pub fn active(mut self, active: bool) -> Self {
        self.core.core.active = active;
        self
    }

    /// Restrict insertions to characters accepted by `accept`
    /// (evaluated after the printable-ASCII bound). Rejected characters
    /// leave buffer and caret untouched (WID-00 §8.2).
    pub fn with_accept<F: Fn(char) -> bool + 'static>(mut self, accept: F) -> Self {
        self.core.core.accept = Some(Box::new(accept));
        self
    }

    /// Cap the buffer at `max` characters (WID-00 §8.1).
    pub fn with_max_len(mut self, max: usize) -> Self {
        self.core.core.max_len = Some(max);
        self
    }

    /// Override the nominal char metrics used for caret geometry
    /// (WID-00 §6.3). Defaults: [`DEFAULT_CHAR_WIDTH`] /
    /// [`DEFAULT_LINE_HEIGHT`].
    pub fn with_char_metrics(mut self, char_width: i32, line_height: i32) -> Self {
        self.core.core.char_width = char_width.max(1);
        self.core.core.line_height = line_height.max(1);
        self
    }

    /// Whether this field currently consumes keys (WID-00 §7).
    pub fn is_active(&self) -> bool {
        self.core.core.active
    }

    /// Toggle key consumption. Applications keep at most one field
    /// active; the framework does not track focus (WID-00 §7.1).
    pub fn set_active(&mut self, active: bool) {
        self.core.core.active = active;
    }

    /// Current caret position (char index, `0..=len`).
    pub fn caret(&self) -> usize {
        self.core.core.caret
    }

    /// Immutable access to the input style.
    pub fn style(&self) -> &rlvgl_core::style::Style {
        &self.core.label.style
    }

    /// Mutable access to the input style.
    pub fn style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        &mut self.core.label.style
    }

    /// Update the input text programmatically and trigger the change
    /// handler if present. The caret clamps to the new length.
    pub fn set_text(&mut self, text: &str) {
        self.core.set_text(text);
    }

    /// Retrieve the current input text.
    pub fn text(&self) -> &str {
        &self.core.core.buffer
    }
}

impl Widget for Input {
    fn bounds(&self) -> Rect {
        self.core.label.bounds()
    }

    fn widget_font_mut(&mut self) -> Option<&mut WidgetFont> {
        self.core.widget_font_mut()
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        self.core.label.draw(renderer);
        self.core.draw_caret(renderer);
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if !self.core.core.active {
            return false;
        }
        if let Event::KeyDown { key } = event {
            if *key == Key::Enter {
                if let Some(cb) = self.on_submit.as_mut() {
                    cb(&self.core.core.buffer);
                }
                return true;
            }
            return self.core.handle_key(key);
        }
        false
    }
}

/// Multi-line editable textarea (WID-00 §5–§8).
///
/// Identical edit model to [`Input`] except Enter inserts a newline
/// (an ordinary edit) instead of submitting, and rendering draws each
/// `'\n'`-split line separately.
pub struct Textarea {
    core: UiEditCore,
}

impl Textarea {
    /// Create a new textarea with the provided text and bounds.
    pub fn new(text: &str, bounds: Rect) -> Self {
        Self {
            core: UiEditCore::new(text, bounds, true),
        }
    }

    /// Assign the font used to render this textarea (FONT-00 §5); resolves to
    /// `FONT_6X10` when unset.
    pub fn set_font(&mut self, font: &'static dyn FontMetrics) {
        self.core.set_font(font);
    }

    /// Register a change handler invoked after every successful edit
    /// and on [`Self::set_text`].
    pub fn on_change<F: FnMut(&str) + 'static>(mut self, handler: F) -> Self {
        self.core.core.on_change = Some(Box::new(handler));
        self
    }

    /// Set whether this textarea consumes key events and return the widget.
    pub fn active(mut self, active: bool) -> Self {
        self.core.core.active = active;
        self
    }

    /// Restrict insertions to characters accepted by `accept`
    /// (newlines from Enter are exempt; WID-00 §8.2).
    pub fn with_accept<F: Fn(char) -> bool + 'static>(mut self, accept: F) -> Self {
        self.core.core.accept = Some(Box::new(accept));
        self
    }

    /// Cap the buffer at `max` characters (newlines count).
    pub fn with_max_len(mut self, max: usize) -> Self {
        self.core.core.max_len = Some(max);
        self
    }

    /// Override the nominal char metrics used for caret geometry and
    /// line pitch (WID-00 §6.3).
    pub fn with_char_metrics(mut self, char_width: i32, line_height: i32) -> Self {
        self.core.core.char_width = char_width.max(1);
        self.core.core.line_height = line_height.max(1);
        self
    }

    /// Whether this field currently consumes keys (WID-00 §7).
    pub fn is_active(&self) -> bool {
        self.core.core.active
    }

    /// Toggle key consumption (WID-00 §7.1).
    pub fn set_active(&mut self, active: bool) {
        self.core.core.active = active;
    }

    /// Current caret position (char index across the whole buffer;
    /// newlines count one).
    pub fn caret(&self) -> usize {
        self.core.core.caret
    }

    /// Immutable access to the textarea style.
    pub fn style(&self) -> &rlvgl_core::style::Style {
        &self.core.label.style
    }

    /// Mutable access to the textarea style.
    pub fn style_mut(&mut self) -> &mut rlvgl_core::style::Style {
        &mut self.core.label.style
    }

    /// Update the textarea text programmatically.
    pub fn set_text(&mut self, text: &str) {
        self.core.set_text(text);
    }

    /// Retrieve the textarea content.
    pub fn text(&self) -> &str {
        &self.core.core.buffer
    }
}

impl Widget for Textarea {
    fn bounds(&self) -> Rect {
        self.core.label.bounds()
    }

    fn widget_font_mut(&mut self) -> Option<&mut WidgetFont> {
        self.core.widget_font_mut()
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        self.core.draw_multi_line(renderer);
        self.core.draw_caret(renderer);
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        if !self.core.core.active {
            return false;
        }
        if let Event::KeyDown { key } = event {
            if *key == Key::Enter {
                self.core.try_insert('\n');
                return true;
            }
            return self.core.handle_key(key);
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::rc::Rc;
    use alloc::vec::Vec;
    use core::cell::{Cell, RefCell};
    use rlvgl_core::widget::Rect;

    const BOUNDS: Rect = Rect {
        x: 0,
        y: 0,
        width: 200,
        height: 20,
    };

    fn key(input: &mut Input, key: Key) -> bool {
        input.handle_event(&Event::KeyDown { key })
    }

    fn type_str(input: &mut Input, s: &str) {
        for c in s.chars() {
            key(input, Key::Character(c));
        }
    }

    #[test]
    fn input_sets_text_and_calls_handler() {
        let called = Rc::new(Cell::new(false));
        let flag = called.clone();
        let mut input = Input::new(
            "hi",
            Rect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            },
        )
        .on_change(move |_| flag.set(true));
        input.set_text("new");
        assert_eq!(input.text(), "new");
        assert!(called.get());
    }

    #[test]
    fn textarea_wraps_input() {
        let mut area = Textarea::new(
            "a",
            Rect {
                x: 0,
                y: 0,
                width: 10,
                height: 20,
            },
        );
        area.set_text("b");
        assert_eq!(area.text(), "b");
    }

    #[test]
    fn synthetic_stream_types_hi_backspace_leaves_h() {
        let mut input = Input::new("", BOUNDS);
        input.set_active(true);
        type_str(&mut input, "HI");
        assert_eq!(input.text(), "HI");
        assert_eq!(input.caret(), 2);
        key(&mut input, Key::Backspace);
        assert_eq!(input.text(), "H");
        assert_eq!(input.caret(), 1);
    }

    #[test]
    fn caret_movement_and_mid_buffer_edits() {
        let mut input = Input::new("", BOUNDS);
        input.set_active(true);
        type_str(&mut input, "AC");
        key(&mut input, Key::ArrowLeft);
        assert_eq!(input.caret(), 1);
        key(&mut input, Key::Character('B'));
        assert_eq!(input.text(), "ABC");
        assert_eq!(input.caret(), 2);
        key(&mut input, Key::Backspace);
        assert_eq!(input.text(), "AC");
        assert_eq!(input.caret(), 1);
        // Clamps at both ends.
        key(&mut input, Key::ArrowLeft);
        key(&mut input, Key::ArrowLeft);
        assert_eq!(input.caret(), 0, "left-clamped");
        key(&mut input, Key::Backspace);
        assert_eq!(input.text(), "AC", "backspace at 0 is a no-op");
        for _ in 0..5 {
            key(&mut input, Key::ArrowRight);
        }
        assert_eq!(input.caret(), 2, "right-clamped");
    }

    #[test]
    fn inactive_inputs_ignore_keys_and_set_active_switches_recipient() {
        let mut a = Input::new("", BOUNDS);
        let mut b = Input::new("", BOUNDS);
        a.set_active(true);

        let press = Event::KeyDown {
            key: Key::Character('x'),
        };
        assert!(a.handle_event(&press));
        assert!(!b.handle_event(&press), "inactive consumes nothing");
        assert_eq!(a.text(), "x");
        assert_eq!(b.text(), "");

        // Application-level focus switch (WID-00 §7.1).
        a.set_active(false);
        b.set_active(true);
        assert!(!a.handle_event(&press));
        assert!(b.handle_event(&press));
        assert_eq!(a.text(), "x", "deactivated field untouched");
        assert_eq!(b.text(), "x");
    }

    #[test]
    fn on_change_fires_per_edit_and_on_submit_on_enter() {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let submits = Rc::new(RefCell::new(Vec::new()));
        let c = changes.clone();
        let s = submits.clone();
        let mut input = Input::new("", BOUNDS)
            .on_change(move |t| c.borrow_mut().push(alloc::string::String::from(t)))
            .on_submit(move |t| s.borrow_mut().push(alloc::string::String::from(t)));
        input.set_active(true);
        type_str(&mut input, "OK");
        key(&mut input, Key::Backspace);
        assert_eq!(*changes.borrow(), alloc::vec!["O", "OK", "O"]);
        key(&mut input, Key::Enter);
        assert_eq!(*submits.borrow(), alloc::vec!["O"]);
        assert_eq!(
            input.text(),
            "O",
            "Enter does not mutate a single-line buffer"
        );
        // Rejected edits fire neither callback.
        key(&mut input, Key::Backspace);
        key(&mut input, Key::Backspace); // second one hits empty buffer
        assert_eq!(changes.borrow().len(), 4);
    }

    #[test]
    fn charset_hook_rejects_without_breaking_caret_state() {
        let mut dialpad =
            Input::new("", BOUNDS).with_accept(|c| c.is_ascii_digit() || c == '*' || c == '#');
        dialpad.set_active(true);
        type_str(&mut dialpad, "1a2*b#");
        assert_eq!(dialpad.text(), "12*#", "out-of-set chars rejected");
        assert_eq!(dialpad.caret(), 4, "caret unaffected by rejections");
        // A rejected char mid-buffer leaves the caret where it was.
        key(&mut dialpad, Key::ArrowLeft);
        key(&mut dialpad, Key::Character('z'));
        assert_eq!(dialpad.caret(), 3);
        key(&mut dialpad, Key::Character('5'));
        assert_eq!(dialpad.text(), "12*5#");
    }

    #[test]
    fn max_len_caps_the_buffer() {
        let mut input = Input::new("", BOUNDS).with_max_len(3);
        input.set_active(true);
        type_str(&mut input, "12345");
        assert_eq!(input.text(), "123");
        assert_eq!(input.caret(), 3);
        key(&mut input, Key::Backspace);
        key(&mut input, Key::Character('9'));
        assert_eq!(input.text(), "129", "frees up after a delete");
    }

    #[test]
    fn non_printable_input_is_rejected() {
        let mut input = Input::new("", BOUNDS);
        input.set_active(true);
        key(&mut input, Key::Character('\u{8}'));
        key(&mut input, Key::Character('\n'));
        key(&mut input, Key::Character('é'));
        assert_eq!(input.text(), "", "ASCII v1 bound (WID-00 §5.2)");
        key(&mut input, Key::Space);
        assert_eq!(input.text(), " ", "Space key inserts a space");
    }

    #[test]
    fn textarea_enter_inserts_newline_and_caret_tracks_rows() {
        let mut area = Textarea::new("", BOUNDS);
        area.set_active(true);
        for c in "AB".chars() {
            area.handle_event(&Event::KeyDown {
                key: Key::Character(c),
            });
        }
        area.handle_event(&Event::KeyDown { key: Key::Enter });
        for c in "C".chars() {
            area.handle_event(&Event::KeyDown {
                key: Key::Character(c),
            });
        }
        assert_eq!(area.text(), "AB\nC");
        assert_eq!(area.caret(), 4);
        assert_eq!(area.core.caret_row_col(), (1, 1));
        // Backspace across the newline merges lines.
        area.handle_event(&Event::KeyDown {
            key: Key::Backspace,
        });
        area.handle_event(&Event::KeyDown {
            key: Key::Backspace,
        });
        assert_eq!(area.text(), "AB");
        assert_eq!(area.core.caret_row_col(), (0, 2));
    }

    /// WID-00 §6.2: rendering is a pure function of the edit state —
    /// typing "HI" then backspace renders byte-identically to typing
    /// just "H" (the sim D-dump test asserts the same over the wire).
    #[test]
    fn render_after_backspace_matches_plain_h_render() {
        struct Capture(Vec<(i32, i32, alloc::string::String)>, Vec<Rect>);
        impl Renderer for Capture {
            fn fill_rect(&mut self, rect: Rect, _color: rlvgl_core::widget::Color) {
                self.1.push(rect);
            }
            fn draw_text(
                &mut self,
                position: (i32, i32),
                text: &str,
                _color: rlvgl_core::widget::Color,
            ) {
                self.0
                    .push((position.0, position.1, alloc::string::String::from(text)));
            }
        }
        let render = |input: &Input| {
            let mut capture = Capture(Vec::new(), Vec::new());
            input.draw(&mut capture);
            (capture.0, capture.1)
        };

        let mut edited = Input::new("", BOUNDS);
        edited.set_active(true);
        type_str(&mut edited, "HI");
        key(&mut edited, Key::Backspace);

        let mut reference = Input::new("", BOUNDS);
        reference.set_active(true);
        type_str(&mut reference, "H");

        assert_eq!(render(&edited), render(&reference));
    }
}
