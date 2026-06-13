//! Collection of built-in widgets for the `rlvgl` toolkit.
#![no_std]
#![deny(missing_docs)]

extern crate alloc;

/// LVGL-parity arc widget.
pub mod arc;
/// LVGL-parity bar widget.
pub mod bar;
/// Clickable button widget.
pub mod button;
/// Grid of labeled buttons arranged in rows.
pub mod button_matrix;
/// Checkbox widget for boolean options.
pub mod checkbox;
/// Transparent click-area widget — rlvgl analogue of QML `MouseArea`.
pub mod click_area;
/// Analog clock widget with sub-pixel anti-aliased hand rotation.
pub mod clock;
/// Container widget for layout grouping.
pub mod container;
/// Image display widget.
pub mod image;
/// State-specific segmented image button.
pub mod image_button;
/// Text label widget.
pub mod label;
/// LVGL-parity LED indicator widget.
pub mod led;
/// Borrowed-slice polyline widget.
pub mod line;
/// Scrollable list widget.
pub mod list;
/// Audio-meter widgets (LED bargraph; needle and others to follow).
pub mod meters;
/// UI motion components (crawls, scrollers, tickers).
pub mod motion;
/// Progress bar widget.
pub mod progress;
/// Radio button widget for mutually exclusive options.
pub mod radio;
/// LVGL-parity tick and label scale widget.
pub mod scale;
/// Scrollable viewport container with parent-bounds child clipping.
pub mod scroll_view;
/// Slider widget for numeric input.
pub mod slider;
/// Numeric text spinbox with range, step, digit format, and rollover.
pub mod spinbox;
/// Deterministic tick-driven spinner widget.
pub mod spinner;
/// Binary on/off switch widget.
pub mod switch;
