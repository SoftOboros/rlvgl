// SPDX-License-Identifier: MIT OR Apache-2.0
//! Event test ensuring Radio toggles when clicked.

use rlvgl_core::{
    event::Event,
    widget::{Rect, Widget},
};
use rlvgl_widgets::radio::Radio;

#[test]
fn radio_event_selects() {
    let rect = Rect {
        x: 0,
        y: 0,
        width: 20,
        height: 20,
    };
    let mut radio = Radio::new("A", rect);
    let event = Event::PointerUp { x: 5, y: 5 };
    assert!(radio.handle_event(&event));
    assert!(radio.is_selected());
}
