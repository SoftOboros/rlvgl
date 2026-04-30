//! Collection of built-in widgets for the `rlvgl` toolkit.
#![no_std]
#![deny(missing_docs)]

extern crate alloc;

/// Clickable button widget.
pub mod button;
/// Checkbox widget for boolean options.
pub mod checkbox;
/// Transparent click-area widget — rlvgl analogue of QML `MouseArea`.
pub mod click_area;
/// Container widget for layout grouping.
pub mod container;
/// Image display widget.
pub mod image;
/// Text label widget.
pub mod label;
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
/// Slider widget for numeric input.
pub mod slider;
/// Binary on/off switch widget.
pub mod switch;
