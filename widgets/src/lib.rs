//! Collection of built-in widgets for the `rlvgl` toolkit.
#![no_std]
#![deny(missing_docs)]

extern crate alloc;

/// Tick-driven animated image (frame sequence).
pub mod anim_image;
/// LVGL-parity arc widget.
pub mod arc;
/// Text laid along an arc (LPAR-Optional).
#[cfg(feature = "lpar_arclabel")]
pub mod arc_label;
/// LVGL-parity bar widget.
pub mod bar;
/// Clickable button widget.
pub mod button;
/// Grid of labeled buttons arranged in rows.
pub mod button_matrix;
/// Month-grid calendar widget.
pub mod calendar;
/// Drawable pixel-buffer canvas widget.
pub mod canvas;
/// Data chart (line/bar/scatter series).
pub mod chart;
/// Checkbox widget for boolean options.
pub mod checkbox;
/// Transparent click-area widget — rlvgl analogue of QML `MouseArea`.
pub mod click_area;
/// Analog clock widget with sub-pixel anti-aliased hand rotation.
pub mod clock;
/// Container widget for layout grouping.
pub mod container;
/// Selection dropdown (closed trigger + open scrollable option list).
pub mod dropdown;
/// Image display widget.
pub mod image;
/// State-specific segmented image button.
pub mod image_button;
/// On-screen keyboard over an internal button matrix.
pub mod keyboard;
/// Text label widget.
pub mod label;
/// LVGL-parity LED indicator widget.
pub mod led;
/// Borrowed-slice polyline widget.
pub mod line;
/// Scrollable list widget.
pub mod list;
/// Page-stack navigation menu.
pub mod menu;
/// Title + message + button-row dialog.
pub mod message_box;
/// Audio-meter widgets (LED bargraph; needle and others to follow).
pub mod meters;
/// UI motion components (crawls, scrollers, tickers).
pub mod motion;
/// Progress bar widget.
pub mod progress;
/// Radio button widget for mutually exclusive options.
pub mod radio;
/// Snap-scrolling wheel selector.
pub mod roller;
/// LVGL-parity tick and label scale widget.
pub mod scale;
/// Scrollable viewport container with parent-bounds child clipping.
pub mod scroll_view;
/// Slider widget for numeric input.
pub mod slider;
/// Rich-text block of styled spans.
pub mod span;
/// Numeric text spinbox with range, step, digit format, and rollover.
pub mod spinbox;
/// Deterministic tick-driven spinner widget.
pub mod spinner;
/// Binary on/off switch widget.
pub mod switch;
/// Row/column data table with shaped-text cells.
pub mod table;
/// Tabbed container with a tab bar and content pages.
pub mod tabview;
/// Multi-line editable text area (LVGL textarea v2).
pub mod textarea;
/// Two-dimensional snap-navigable tile grid.
pub mod tileview;
/// Title-bar + content-area window container.
pub mod window;
