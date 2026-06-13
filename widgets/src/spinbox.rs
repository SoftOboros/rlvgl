//! Numeric text spinbox with range, step, digit format, and rollover (LPAR-12c).
//!
//! [`Spinbox`] stores an integer value with a configurable digit count, decimal
//! separator position, step size, and range.  The value is formatted as a
//! right-aligned string with leading zeros, an optional sign column, and an
//! optional decimal separator; callers interpret decimal placement.
//!
//! # Key helpers
//!
//! Wire [`rlvgl_core::event::Event::KeyDown`] in the containing node handler:
//!
//! * `ArrowUp` → [`increment`](Spinbox::increment)
//! * `ArrowDown` → [`decrement`](Spinbox::decrement)
//! * `ArrowRight` → [`on_arrow_right`](Spinbox::on_arrow_right)
//! * `ArrowLeft` → [`on_arrow_left`](Spinbox::on_arrow_left)

use alloc::string::String;
use core::fmt::Write as FmtWrite;
use rlvgl_core::bitmap_font::FONT_6X10;
use rlvgl_core::draw::draw_widget_bg;
use rlvgl_core::event::Event;
use rlvgl_core::font::{FontMetrics, shape_text_ltr};
use rlvgl_core::renderer::{ClipRenderer, Renderer};
use rlvgl_core::style::Style;
use rlvgl_core::widget::{Color, Rect, Widget};

/// Direction of cursor movement when stepping through digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpinboxDigitStepDirection {
    /// Move cursor to the right (toward less significant digits) when stepping.
    Right,
    /// Move cursor to the left (toward more significant digits) when stepping.
    Left,
}

/// Numeric text-editing spinbox.
///
/// Stores an integer value with a configurable digit count, decimal separator
/// position, step size, and range.  The value is formatted as a string with
/// leading zeros and an optional sign column; callers interpret decimal placement.
///
/// # Defaults
///
/// * `value`: 0
/// * `digit_count`: 5
/// * `separator_position`: 0 (hidden)
/// * `step`: 1
/// * range: `−99999..=99999`
/// * `rollover`: disabled
/// * `digit_step_direction`: right
pub struct Spinbox {
    bounds: Rect,
    value: i32,
    min: i32,
    max: i32,
    rollover: bool,
    digit_count: u8,
    separator_position: u8,
    step: i32,
    digit_step_direction: SpinboxDigitStepDirection,
    /// Visual style for the spinbox background.
    pub style: Style,
    /// Color used to render the numeric text.
    pub text_color: Color,
    /// Color used for the selected-digit cursor underline.
    pub cursor_color: Color,
}

impl Spinbox {
    /// Create a spinbox with default LVGL-parity settings occupying `bounds`.
    pub fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            value: 0,
            min: -99_999,
            max: 99_999,
            rollover: false,
            digit_count: 5,
            separator_position: 0,
            step: 1,
            digit_step_direction: SpinboxDigitStepDirection::Right,
            style: Style::default(),
            text_color: Color(20, 20, 20, 255),
            cursor_color: Color(0, 120, 215, 255),
        }
    }

    /// Set the current value, clamping to the configured range.
    pub fn set_value(&mut self, value: i32) {
        self.value = value.clamp(self.min, self.max);
    }

    /// Return the current value.
    pub fn value(&self) -> i32 {
        self.value
    }

    /// Set the inclusive value range and clamp the current value.
    pub fn set_range(&mut self, min: i32, max: i32) {
        self.min = min;
        self.max = max;
        self.value = self.value.clamp(self.min, self.max);
    }

    /// Return the configured minimum value.
    pub fn min_value(&self) -> i32 {
        self.min
    }

    /// Return the configured maximum value.
    pub fn max_value(&self) -> i32 {
        self.max
    }

    /// Enable or disable rollover at range boundaries.
    pub fn set_rollover(&mut self, rollover: bool) {
        self.rollover = rollover;
    }

    /// Return `true` when rollover is enabled.
    pub fn rollover(&self) -> bool {
        self.rollover
    }

    /// Configure the digit count and decimal separator position.
    ///
    /// `digit_count` is clamped to `1..=10`; it excludes the sign and
    /// separator character.  `separator_position == 0` or
    /// `separator_position >= digit_count` hides the separator.  Setting the
    /// format tightens the representable range to
    /// `−(10^digit_count − 1)..=(10^digit_count − 1)` and clamps the value.
    pub fn set_digit_format(&mut self, digit_count: u8, separator_position: u8) {
        self.digit_count = digit_count.clamp(1, 10);
        self.separator_position = separator_position;
        let max_repr = max_for_digits(self.digit_count);
        self.min = self.min.clamp(-max_repr, max_repr);
        self.max = self.max.clamp(-max_repr, max_repr);
        self.value = self.value.clamp(self.min, self.max);
    }

    /// Return the configured digit count (excluding sign and separator).
    pub fn digit_count(&self) -> u8 {
        self.digit_count
    }

    /// Return the separator position (0 = hidden).
    pub fn separator_position(&self) -> u8 {
        self.separator_position
    }

    /// Set the step size for increment/decrement operations (minimum 1).
    pub fn set_step(&mut self, step: i32) {
        self.step = step.max(1);
    }

    /// Return the current step size.
    pub fn step(&self) -> i32 {
        self.step
    }

    /// Set the direction for digit-step cursor movement.
    pub fn set_digit_step_direction(&mut self, dir: SpinboxDigitStepDirection) {
        self.digit_step_direction = dir;
    }

    /// Return the digit-step direction.
    pub fn digit_step_direction(&self) -> SpinboxDigitStepDirection {
        self.digit_step_direction
    }

    /// Move the selected digit toward more significant digits.
    ///
    /// Multiplies the step by 10, capped at `10^(digit_count − 1)`.
    pub fn step_next(&mut self) {
        let cap = max_step(self.digit_count);
        if self.step < cap {
            self.step = (self.step.saturating_mul(10)).min(cap);
        }
    }

    /// Move the selected digit toward less significant digits.
    ///
    /// Divides the step by 10, floored at `1`.
    pub fn step_prev(&mut self) {
        if self.step > 1 {
            self.step /= 10;
        }
    }

    /// Increment the value by the current step, respecting range and rollover.
    ///
    /// LVGL zero-crossing behavior: when the step carries the value from
    /// negative to positive the value lands on `0` first.
    pub fn increment(&mut self) {
        let v = self.value;
        let s = self.step;
        let new = if v < 0 && v.saturating_add(s) > 0 {
            0
        } else {
            v.saturating_add(s)
        };
        if new > self.max {
            self.value = if self.rollover { self.min } else { self.max };
        } else {
            self.value = new;
        }
    }

    /// Decrement the value by the current step, respecting range and rollover.
    ///
    /// LVGL zero-crossing behavior: when the step carries the value from
    /// positive to negative the value lands on `0` first.
    pub fn decrement(&mut self) {
        let v = self.value;
        let s = self.step;
        let new = if v > 0 && v.saturating_sub(s) < 0 {
            0
        } else {
            v.saturating_sub(s)
        };
        if new < self.min {
            self.value = if self.rollover { self.max } else { self.min };
        } else {
            self.value = new;
        }
    }

    /// Increment via `ArrowUp` key — convenience wrapper for node handlers.
    pub fn on_arrow_up(&mut self) {
        self.increment();
    }

    /// Decrement via `ArrowDown` key — convenience wrapper.
    pub fn on_arrow_down(&mut self) {
        self.decrement();
    }

    /// Move cursor right (toward less significant digit) via `ArrowRight` key.
    pub fn on_arrow_right(&mut self) {
        match self.digit_step_direction {
            SpinboxDigitStepDirection::Right => self.step_prev(),
            SpinboxDigitStepDirection::Left => self.step_next(),
        }
    }

    /// Move cursor left (toward more significant digit) via `ArrowLeft` key.
    pub fn on_arrow_left(&mut self) {
        match self.digit_step_direction {
            SpinboxDigitStepDirection::Right => self.step_next(),
            SpinboxDigitStepDirection::Left => self.step_prev(),
        }
    }

    /// Return the formatted numeric string.
    ///
    /// Format: optional sign column, `digit_count` digits with leading zeros,
    /// and an optional decimal separator inserted `separator_position` places
    /// from the right.  Negative-capable ranges (`min < 0`) prefix `−` for
    /// negative values and `+` for positives; positive-only ranges omit the
    /// sign column entirely.
    pub fn text(&self) -> String {
        let dc = self.digit_count as usize;
        let sep = self.separator_position as usize;
        let show_sign = self.min < 0;
        let abs_val = self.value.unsigned_abs() as u64;

        // Build digit characters (most-significant first)
        let mut digits = [b'0'; 10];
        let mut temp = abs_val;
        for i in (0..dc).rev() {
            digits[i] = b'0' + (temp % 10) as u8;
            temp /= 10;
        }

        let mut s = String::new();

        if show_sign {
            let sign = if self.value < 0 { '-' } else { '+' };
            s.push(sign);
        }

        let sep_valid = sep > 0 && sep < dc;
        let sep_insert_before = dc - sep; // insert '.' before digit at this index from start

        for (i, &digit) in digits[..dc].iter().enumerate() {
            if sep_valid && i == sep_insert_before {
                s.push('.');
            }
            s.push(digit as char);
        }

        s
    }

    /// Return the character index in [`text()`](Self::text) for the digit
    /// selected by the current step.
    ///
    /// Used internally by [`draw`](Widget::draw) to position the cursor underline.
    fn selected_char_index(&self) -> usize {
        let dc = self.digit_count as usize;
        let sep = self.separator_position as usize;
        let show_sign = self.min < 0;

        // Which digit from the right is selected?
        let digit_from_right = {
            let mut d = 0usize;
            let mut s = self.step;
            while s > 1 {
                s /= 10;
                d += 1;
            }
            d
        };
        // Convert to index from left in the digit array (0 = most significant)
        let digit_from_left = dc.saturating_sub(1).saturating_sub(digit_from_right);

        // Account for sign prefix and separator character
        let sep_valid = sep > 0 && sep < dc;
        let sep_insert_before = dc - sep;
        let sep_before = if sep_valid && digit_from_left >= sep_insert_before {
            1
        } else {
            0
        };
        let sign_offset = if show_sign { 1 } else { 0 };

        sign_offset + digit_from_left + sep_before
    }
}

fn max_for_digits(digit_count: u8) -> i32 {
    let n = digit_count.min(9) as u32; // i32::MAX ~ 2.1e9; 10^9 fits
    let mut v: i32 = 1;
    for _ in 0..n {
        v = v.saturating_mul(10);
    }
    v.saturating_sub(1)
}

fn max_step(digit_count: u8) -> i32 {
    // max step = 10^(digit_count - 1)
    let n = digit_count.saturating_sub(1).min(9) as u32;
    let mut v: i32 = 1;
    for _ in 0..n {
        v = v.saturating_mul(10);
    }
    v
}

impl Widget for Spinbox {
    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn draw(&self, renderer: &mut dyn Renderer) {
        draw_widget_bg(renderer, self.bounds, &self.style);

        let text = self.text();
        let font: &dyn FontMetrics = &FONT_6X10;
        let metrics = font.line_metrics();
        let baseline = self.bounds.y + metrics.ascent as i32;
        let shaped = shape_text_ltr(font, &text, (self.bounds.x, baseline), 0);
        let color = self.text_color.with_alpha(self.style.alpha);

        {
            let mut clipped = ClipRenderer::new(renderer, self.bounds);
            clipped.draw_text_shaped(&shaped, (0, 0), color);
        }

        // Draw cursor underline under the selected digit
        let char_idx = self.selected_char_index();
        if let Some(glyph) = shaped.glyphs.get(char_idx) {
            let ext = glyph.extent();
            let cursor_rect = Rect {
                x: ext.x,
                y: ext.y + ext.height,
                width: ext.width.max(1),
                height: 2,
            };
            renderer.fill_rect(cursor_rect, self.cursor_color.with_alpha(self.style.alpha));
        }
    }

    fn handle_event(&mut self, _event: &Event) -> bool {
        // Key navigation is wired by the app via ObjectEvent::Key node handlers;
        // raw key events are not handled here (LPAR-12 §5.E).
        false
    }
}

// Suppress unused-import warning for FmtWrite which is only used implicitly
// through the String::write_fmt call generated by format! / write! below.
// We use manual digit assembly so FmtWrite is not actually referenced directly;
// keep the import to document intent and suppress the lint in that scenario.
// Actually we don't use write! here — remove it.
#[allow(unused_imports)]
use FmtWrite as _;

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    #[test]
    fn default_value_and_range() {
        let sb = Spinbox::new(rect(0, 0, 80, 20));
        assert_eq!(sb.value(), 0);
        assert_eq!(sb.min_value(), -99_999);
        assert_eq!(sb.max_value(), 99_999);
        assert_eq!(sb.digit_count(), 5);
        assert_eq!(sb.separator_position(), 0);
        assert_eq!(sb.step(), 1);
        assert!(!sb.rollover());
    }

    #[test]
    fn set_value_clamps_to_range() {
        let mut sb = Spinbox::new(rect(0, 0, 80, 20));
        sb.set_value(200_000);
        assert_eq!(sb.value(), 99_999);
        sb.set_value(-200_000);
        assert_eq!(sb.value(), -99_999);
    }

    #[test]
    fn set_range_clamps_value() {
        let mut sb = Spinbox::new(rect(0, 0, 80, 20));
        sb.set_value(50);
        sb.set_range(0, 30);
        assert_eq!(sb.value(), 30);
    }

    #[test]
    fn digit_format_shows_plus_and_leading_zeros() {
        let mut sb = Spinbox::new(rect(0, 0, 80, 20));
        sb.set_value(42);
        let t = sb.text();
        assert!(t.starts_with('+'), "expected '+' prefix, got: {t}");
        assert!(t.contains("00042"), "expected leading zeros in: {t}");
    }

    #[test]
    fn positive_only_range_omits_sign() {
        let mut sb = Spinbox::new(rect(0, 0, 80, 20));
        sb.set_range(0, 99_999);
        sb.set_value(5);
        let t = sb.text();
        assert!(
            !t.starts_with('+') && !t.starts_with('-'),
            "unexpected sign prefix in: {t}"
        );
        assert!(t.contains("00005"), "expected leading zeros in: {t}");
    }

    #[test]
    fn negative_value_shows_minus() {
        let mut sb = Spinbox::new(rect(0, 0, 80, 20));
        sb.set_value(-123);
        let t = sb.text();
        assert!(t.starts_with('-'), "expected '-' prefix, got: {t}");
    }

    #[test]
    fn separator_position_inserts_dot() {
        let mut sb = Spinbox::new(rect(0, 0, 80, 20));
        sb.set_digit_format(5, 2); // dot 2 from the right: "NNN.NN"
        sb.set_value(12345);
        let t = sb.text();
        assert!(t.contains('.'), "expected '.' separator in: {t}");
    }

    #[test]
    fn separator_zero_hides_dot() {
        let mut sb = Spinbox::new(rect(0, 0, 80, 20));
        sb.set_digit_format(5, 0);
        sb.set_value(123);
        assert!(!sb.text().contains('.'), "unexpected '.' in: {}", sb.text());
    }

    #[test]
    fn increment_decrement_basic() {
        let mut sb = Spinbox::new(rect(0, 0, 80, 20));
        sb.set_value(10);
        sb.increment();
        assert_eq!(sb.value(), 11);
        sb.decrement();
        sb.decrement();
        assert_eq!(sb.value(), 9);
    }

    #[test]
    fn zero_crossing_increment_lands_on_zero() {
        let mut sb = Spinbox::new(rect(0, 0, 80, 20));
        sb.set_step(5);
        sb.set_value(-3);
        sb.increment(); // -3 + 5 = 2 > 0  → land at 0
        assert_eq!(sb.value(), 0);
    }

    #[test]
    fn zero_crossing_decrement_lands_on_zero() {
        let mut sb = Spinbox::new(rect(0, 0, 80, 20));
        sb.set_step(5);
        sb.set_value(3);
        sb.decrement(); // 3 - 5 = -2 < 0  → land at 0
        assert_eq!(sb.value(), 0);
    }

    #[test]
    fn rollover_wraps_at_max() {
        let mut sb = Spinbox::new(rect(0, 0, 80, 20));
        sb.set_range(0, 10);
        sb.set_rollover(true);
        sb.set_value(10);
        sb.increment();
        assert_eq!(sb.value(), 0);
    }

    #[test]
    fn rollover_wraps_at_min() {
        let mut sb = Spinbox::new(rect(0, 0, 80, 20));
        sb.set_range(0, 10);
        sb.set_rollover(true);
        sb.set_value(0);
        sb.decrement();
        assert_eq!(sb.value(), 10);
    }

    #[test]
    fn no_rollover_clamps_at_max() {
        let mut sb = Spinbox::new(rect(0, 0, 80, 20));
        sb.set_range(0, 10);
        sb.set_value(10);
        sb.increment();
        assert_eq!(sb.value(), 10);
    }

    #[test]
    fn step_next_multiplies_by_ten() {
        let mut sb = Spinbox::new(rect(0, 0, 80, 20));
        assert_eq!(sb.step(), 1);
        sb.step_next();
        assert_eq!(sb.step(), 10);
        sb.step_next();
        assert_eq!(sb.step(), 100);
    }

    #[test]
    fn step_prev_divides_by_ten() {
        let mut sb = Spinbox::new(rect(0, 0, 80, 20));
        sb.set_step(100);
        sb.step_prev();
        assert_eq!(sb.step(), 10);
        sb.step_prev();
        assert_eq!(sb.step(), 1);
        sb.step_prev(); // floor at 1
        assert_eq!(sb.step(), 1);
    }

    #[test]
    fn step_next_capped_at_digit_count_max() {
        let mut sb = Spinbox::new(rect(0, 0, 80, 20));
        sb.set_digit_format(3, 0); // max step = 100
        for _ in 0..10 {
            sb.step_next();
        }
        assert_eq!(sb.step(), 100);
    }

    #[test]
    fn set_bounds_adopted() {
        let mut sb = Spinbox::new(rect(0, 0, 80, 20));
        sb.set_bounds(rect(5, 5, 100, 30));
        assert_eq!(sb.bounds(), rect(5, 5, 100, 30));
    }

    #[test]
    fn digit_format_clamps_digit_count() {
        let mut sb = Spinbox::new(rect(0, 0, 80, 20));
        sb.set_digit_format(20, 0);
        assert_eq!(sb.digit_count(), 10);
    }

    #[test]
    fn handle_event_always_returns_false() {
        let mut sb = Spinbox::new(rect(0, 0, 80, 20));
        assert!(!sb.handle_event(&Event::PressRelease { x: 10, y: 10 }));
    }
}
